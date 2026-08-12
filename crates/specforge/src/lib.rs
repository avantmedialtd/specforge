mod commands;
#[cfg(target_os = "macos")]
mod dock_badge;
mod events;
#[cfg(target_os = "macos")]
mod menu;
mod notifications;
mod tray;
mod tray_icon;

use tauri::Manager;
use tray_icon::{TrayGlyph, TrayGlyphState};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        // Opens the OS default handler for a validated artifact link. Rust-only
        // (see `commands::open_artifact_link`) — no JS package, no `opener:*`
        // capability permission.
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Build the main window ourselves — its `tauri.conf.json` entry
            // sets `"create": false` — so an `on_navigation` guard is attached
            // before the webview ever loads anything. This is the backstop for
            // activation paths no DOM click handler can see (the webview's
            // native "Open Link" context-menu item, link drag-out) and for any
            // future renderer regression: only the app's own origin may load —
            // the production custom-protocol origin (`tauri://…`; this app
            // never sets `useHttpsScheme`, so the scheme stays `tauri` rather
            // than the `https://tauri.localhost` Windows workaround form), or,
            // in a `bun tauri dev` build only, the local dev server regardless
            // of port (`bun run wt:dev`'s worktree-slot mechanism varies the
            // port per worktree; `cfg!(dev)` keeps the relaxation out of
            // release builds). This is the exact recipe `WebviewWindowBuilder::
            // on_navigation`'s own doc example demonstrates. Built first,
            // before anything else in setup(), so the window appears exactly
            // as early as it did when Tauri created it automatically.
            let window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .expect("the \"main\" window must be declared in tauri.conf.json");
            let main_window =
                tauri::WebviewWindowBuilder::from_config(app.handle(), &window_config)?
                    .on_navigation(|url| {
                        url.scheme() == "tauri"
                            || (cfg!(dev) && url.host_str() == Some("localhost"))
                    })
                    .build()?;

            // macOS: install our own application menu so the "About SpecForge"
            // item can carry an enriched About panel (name, version, copyright,
            // and a credits block with the tagline / repo URL / license — the
            // native panel only renders those fields; see `menu.rs`). Setting a
            // menu replaces Tauri's auto-default, so `build_app_menu` also
            // rebuilds the standard Edit/Window submenus to keep Cmd-C/V/X/A and
            // Cmd-M working. macOS-only: on Windows/Linux a custom Menu renders
            // as a window menu bar, which is wrong for a tray-resident app.
            // Installed first so it is in place before the window is shown;
            // independent of the cache and event-forwarder ordering below.
            #[cfg(target_os = "macos")]
            {
                let app_menu = menu::build_app_menu(app.handle())?;
                app.handle().set_menu(app_menu)?;
                app.handle()
                    .on_menu_event(|handle, event| menu::handle_menu_event(handle, &event));
            }

            // Resolve the app-data directory through the shared resolver in
            // `openspec-app` rather than Tauri's `app_config_dir()`, so the
            // desktop shell and the terminal frontend (which has no Tauri)
            // agree on one path. `openspec_app::config_dir()` is the same
            // computation Tauri v2 performs (`dirs::config_dir()` + identifier),
            // so this is path-preserving for the desktop app.
            let config_dir = openspec_app::config_dir().expect("app config dir must be resolvable");

            // The headless application service owns the registry, settings,
            // presentation store, activity log, and watcher, plus first-run
            // identity seeding. The terminal frontend builds the same
            // service; the shell layers only its OS integration (tray,
            // notifications, dock badge, menu) on top.
            let svc = openspec_app::AppService::bootstrap(config_dir);

            // Forward CacheEvents → Tauri events before any cache population so
            // we don't miss the populate-event burst (initial add_workspace
            // calls do not emit Updated, but subsequent filesystem changes do).
            events::spawn_event_forwarder(app.handle().clone(), &svc.watcher);

            // Synchronously populate the cache for previously-registered
            // workspaces so the frontend's first request sees a consistent
            // cache instead of racing an in-flight populate. Must run inside a
            // tokio context (`add_workspace` / `sync_repos` spawn tasks).
            tauri::async_runtime::block_on(svc.populate());

            // Seed historical achievements from git on first launch (gated on an
            // empty log), off-thread; nudges each repo's graph when done so an
            // open Dashboard refetches the now-seeded log.
            svc.spawn_backfill();

            // Start the opt-in Claude usage-quota poll loop on its own thread.
            // It is a no-op (only re-checks the flag, never hits the network)
            // until the user enables the feature from Settings.
            svc.spawn_quota_poller();

            // Twin poller for the opt-in ChatGPT usage-quota gauge — same
            // no-op-while-disabled posture as the Claude poller above.
            svc.spawn_chatgpt_quota_poller();

            // Optional embedded web UI: when enabled in settings, serve the
            // browser skin from THIS `AppService` — so the web view mirrors the
            // desktop's live state through one watcher with no second writer.
            // Loopback-only; read once at launch (the toggle persists and takes
            // effect on the next start).
            {
                let web = svc.settings.web_config();
                if web.enabled {
                    let web_svc = svc.clone();
                    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], web.port));
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = specforge_web::serve(web_svc, addr).await {
                            eprintln!("embedded web server error: {e}");
                        }
                    });
                }
            }

            // Install the system tray icon and start its badge updater.
            // Must happen after the cache is populated so the initial badge
            // count reflects the registered workspaces.
            //
            // The tray glyph is rasterized from an SVG at the active monitor's
            // pixel density. If the primary monitor can't be queried (rare —
            // e.g., headless), fall back to 1.0 and accept a soft icon.
            let monitor_scale = app
                .primary_monitor()?
                .map(|m| m.scale_factor())
                .unwrap_or(1.0);

            // Seed the initial glyph variant from the populated cache so the
            // first painted icon already reflects spec activity. `TrayGlyphState`
            // MUST be `manage()`-d before the window event handler below is
            // registered — the `ScaleFactorChanged` arm reads it to know which
            // SVG to re-rasterize.
            let initial_variant = if svc.watcher.any_change_touches_specs() {
                TrayGlyph::Specs
            } else {
                TrayGlyph::Default
            };
            let glyph_state = TrayGlyphState::new(initial_variant);
            app.manage(glyph_state.clone());

            let tray_handle = tray::install_tray(app.handle(), monitor_scale, initial_variant)?;
            tray::spawn_badge_updater(tray_handle.clone(), svc.watcher.clone());
            tray::spawn_tray_glyph_updater(
                tray_handle,
                app.handle().clone(),
                svc.watcher.clone(),
                glyph_state,
                monitor_scale,
            );

            // Desktop-notification dispatcher subscribes to the same
            // CacheEvent stream as the forwarder and badge updater; gated
            // by the in-app notifications-enabled setting.
            notifications::spawn_notification_dispatcher(
                app.handle().clone(),
                &svc.watcher,
                svc.settings.clone(),
                svc.registry.clone(),
                svc.presentation.clone(),
            );

            // Close button hides the main window instead of destroying it,
            // so the watcher and tray icon keep working. Cmd-Q (or the
            // "Quit" tray menu item) is the only exit path.
            //
            // Also: when the window moves to a display with a different scale
            // factor, re-rasterize the tray glyph so it stays crisp.
            //
            // `main_window` was built explicitly above (rather than looked up
            // via `get_webview_window`) — it's already guaranteed to exist.
            {
                // Dock-badge updater: mirrors the tray badge onto the macOS
                // Dock tile (and therefore the CMD+Tab switcher). Initial
                // call inside the spawned task sees the cache already
                // populated by the synchronous block_on above, so the badge
                // is correct on first paint.
                #[cfg(target_os = "macos")]
                dock_badge::spawn_dock_badge_updater(main_window.clone(), svc.watcher.clone());

                let window_for_event = main_window.clone();
                // Recompute working-tree status when the window regains focus.
                // This is the backstop for the whole-repo dirty rollup: a
                // non-spec edit made while the app was unfocused touches neither
                // a spec file (openspec watcher) nor `.git/index` (repo-monitor
                // index watcher), so focus is when we re-scan. Run off the UI
                // thread — `refresh_status_and_notify` shells out to `git
                // status` per worktree.
                let watcher_for_focus = svc.watcher.clone();
                main_window.on_window_event(move |event| match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window_for_event.hide();
                    }
                    tauri::WindowEvent::Focused(true) => {
                        let watcher = watcher_for_focus.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            watcher.refresh_status_and_notify();
                        });
                    }
                    tauri::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        let app = window_for_event.app_handle();
                        if let Some(tray) = app.tray_by_id(tray::TRAY_ID) {
                            let variant = app.state::<TrayGlyphState>().load();
                            let icon = tray_icon::rasterize_glyph(variant, *scale_factor);
                            let _ = tray.set_icon_with_as_template(Some(icon), true);
                        }
                    }
                    _ => {}
                });
            }

            // Manage the individual handles for the commands that still take
            // them directly (registration, presentation writes, settings, the
            // remaining read commands), plus the whole `AppService` for the
            // commands that delegate to it (dashboard, garden). All clones
            // share the same underlying state.
            app.manage(svc.registry.clone());
            app.manage(svc.settings.clone());
            app.manage(svc.presentation.clone());
            app.manage(svc.watcher.clone());
            app.manage(svc);

            #[cfg(debug_assertions)]
            {
                if std::env::var_os("SPECFORGE_OPEN_DEVTOOLS").is_some() {
                    main_window.open_devtools();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::register_workspace,
            commands::unregister_workspace,
            commands::list_workspaces,
            commands::get_changes,
            commands::list_archived,
            commands::archived_artifact_status,
            commands::get_workspace_views,
            commands::get_active_count,
            commands::get_dashboard,
            commands::get_commit_garden,
            commands::read_artifact,
            commands::list_markdown_files,
            commands::read_workspace_file,
            commands::open_artifact_link,
            commands::get_commit_graph,
            commands::get_commit_detail,
            commands::get_commit_diff,
            commands::get_launch_on_login,
            commands::set_launch_on_login,
            commands::get_notifications_enabled,
            commands::set_notifications_enabled,
            commands::get_collapsed_tree_node_ids,
            commands::set_collapsed_tree_node_ids,
            commands::get_expanded_tree_node_ids,
            commands::set_expanded_tree_node_ids,
            commands::get_favorite_change_ids,
            commands::update_favorite_change_ids,
            commands::set_workspace_presentation,
            commands::set_workspace_disabled,
            commands::get_identity,
            commands::set_display_name,
            commands::set_identity_aliases,
            commands::set_people,
            commands::observed_authors,
            commands::get_claude_quota,
            commands::get_claude_quota_enabled,
            commands::set_claude_quota_enabled,
            commands::get_chatgpt_quota,
            commands::get_chatgpt_quota_enabled,
            commands::set_chatgpt_quota_enabled,
            commands::get_wsl_poll_interval_secs,
            commands::set_wsl_poll_interval_secs,
            commands::get_web_config,
            commands::set_web_enabled,
            commands::set_web_port,
            commands::set_web_tailscale_enabled,
            commands::set_web_tailscale_name,
            commands::set_web_tailscale_allowed_logins,
            commands::resolve_tailscale_name,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // macOS: when the user clicks the Dock icon and no windows are
            // visible (because they closed the only window, which we turn
            // into "hide"), bring the main window back. `RunEvent::Reopen`
            // is macOS-only; the cfg guard keeps this from breaking the
            // Linux CI build.
            #[cfg(target_os = "macos")]
            {
                if let tauri::RunEvent::Reopen {
                    has_visible_windows: false,
                    ..
                } = event
                {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (app_handle, event);
            }
        });
}
