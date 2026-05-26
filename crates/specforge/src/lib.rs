mod commands;
#[cfg(target_os = "macos")]
mod dock_badge;
mod events;
mod notifications;
mod settings;
mod tray;
mod tray_icon;

use openspec_core::{WatcherManager, WorkspacePresentationStore, WorkspaceRegistry};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tray_icon::{TrayGlyph, TrayGlyphState};

#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .expect("app config dir must be resolvable");
            std::fs::create_dir_all(&config_dir).ok();

            let workspaces_path = config_dir.join("workspaces.json");
            let settings_path = config_dir.join("settings.json");
            let presentation_path = config_dir.join("presentation.json");

            let registry = WorkspaceRegistry::load(workspaces_path.clone())
                .unwrap_or_else(|_| WorkspaceRegistry::new(workspaces_path));
            let settings = Arc::new(settings::SettingsStore::load(settings_path));
            // Per-workspace display-name and tint overrides. Missing file is
            // a valid empty store; a corrupt file falls back to empty so the
            // app still launches.
            let presentation = WorkspacePresentationStore::load(presentation_path.clone())
                .unwrap_or_else(|_| WorkspacePresentationStore::new(presentation_path));
            let shared_presentation = Arc::new(Mutex::new(presentation));
            // Share the registry with the WatcherManager so the meta-watcher
            // can reconcile the discovered-worktree set on `.git/worktrees/`
            // events. The lib-default debounce is fine here.
            let shared_registry = Arc::new(Mutex::new(registry));
            let watcher = WatcherManager::with_registry(
                std::time::Duration::from_millis(200),
                Some(shared_registry.clone()),
            );

            // Forward CacheEvents → Tauri events before any cache population so
            // we don't miss the populate-event burst (initial add_workspace
            // calls do not emit Updated, but subsequent filesystem changes do).
            events::spawn_event_forwarder(app.handle().clone(), &watcher);

            // Synchronously populate the cache for previously-registered
            // workspaces. The frontend's first `get_changes` call then sees a
            // consistent cache instead of racing against an in-flight populate.
            // Missing folders are skipped (the registry already marks them).
            // `folders()` includes auto-discovered worktrees re-derived by
            // `WorkspaceRegistry::load`, so every tracked workspace is wired
            // up here.
            let folders = shared_registry.lock().unwrap().folders();
            let watcher_for_setup = watcher.clone();
            // Every watcher-setup call that spawns tasks (`add_workspace`,
            // `sync_repos` → `RepoMonitor::install`, `spawn_aggregator`) must
            // run inside an active tokio context. Group them under one
            // `block_on` rather than calling some from sync code afterward,
            // which would panic with "there is no reactor running".
            tauri::async_runtime::block_on(async move {
                for folder in folders {
                    if folder.uri.is_dir() {
                        if let Err(e) = watcher_for_setup.add_workspace(folder).await {
                            eprintln!("failed to start watcher: {e}");
                        }
                    }
                }
                // Install repo monitors for every distinct repo present in
                // the registry. Picks up runtime worktree adds/removes on
                // `.git/worktrees/` and refreshes the cached default branch
                // on `.git/config` / `origin/HEAD` changes.
                watcher_for_setup.sync_repos();
                // Wire up the aggregator: it subscribes to raw cache events,
                // recomputes the aggregated view, and emits logical/instance
                // diff events. Initial aggregation here so the first
                // `get_workspace_views` request returns a populated snapshot.
                watcher_for_setup.aggregate_and_emit();
                watcher_for_setup.spawn_aggregator();
            });

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
            let initial_variant = if watcher.any_change_touches_specs() {
                TrayGlyph::Specs
            } else {
                TrayGlyph::Default
            };
            let glyph_state = TrayGlyphState::new(initial_variant);
            app.manage(glyph_state.clone());

            let tray_handle = tray::install_tray(app.handle(), monitor_scale, initial_variant)?;
            tray::spawn_badge_updater(tray_handle.clone(), watcher.clone());
            tray::spawn_tray_glyph_updater(
                tray_handle,
                app.handle().clone(),
                watcher.clone(),
                glyph_state,
                monitor_scale,
            );

            // Desktop-notification dispatcher subscribes to the same
            // CacheEvent stream as the forwarder and badge updater; gated
            // by the in-app notifications-enabled setting.
            notifications::spawn_notification_dispatcher(
                app.handle().clone(),
                &watcher,
                settings.clone(),
            );

            // Close button hides the main window instead of destroying it,
            // so the watcher and tray icon keep working. Cmd-Q (or the
            // "Quit" tray menu item) is the only exit path.
            //
            // Also: when the window moves to a display with a different scale
            // factor, re-rasterize the tray glyph so it stays crisp.
            if let Some(main_window) = app.get_webview_window("main") {
                // macOS: apply NSVisualEffectMaterial::Sidebar so the
                // sidebar shows desktop vibrancy through transparent CSS.
                // Fault-tolerant — a failure on an older OS version leaves
                // the window solid but doesn't block startup.
                #[cfg(target_os = "macos")]
                if let Err(err) =
                    apply_vibrancy(&main_window, NSVisualEffectMaterial::Sidebar, None, None)
                {
                    eprintln!("sidebar vibrancy not applied: {err}");
                }

                // Dock-badge updater: mirrors the tray badge onto the macOS
                // Dock tile (and therefore the CMD+Tab switcher). Initial
                // call inside the spawned task sees the cache already
                // populated by the synchronous block_on above, so the badge
                // is correct on first paint.
                #[cfg(target_os = "macos")]
                dock_badge::spawn_dock_badge_updater(main_window.clone(), watcher.clone());

                let window_for_event = main_window.clone();
                main_window.on_window_event(move |event| match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window_for_event.hide();
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

            app.manage(shared_registry);
            app.manage(watcher);
            app.manage(settings);
            app.manage(shared_presentation);

            #[cfg(debug_assertions)]
            {
                if std::env::var_os("SPECFORGE_OPEN_DEVTOOLS").is_some() {
                    if let Some(window) = app.get_webview_window("main") {
                        window.open_devtools();
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::register_workspace,
            commands::unregister_workspace,
            commands::list_workspaces,
            commands::get_changes,
            commands::get_workspace_views,
            commands::get_active_count,
            commands::read_artifact,
            commands::get_launch_on_login,
            commands::set_launch_on_login,
            commands::get_notifications_enabled,
            commands::set_notifications_enabled,
            commands::get_collapsed_tree_node_ids,
            commands::set_collapsed_tree_node_ids,
            commands::set_workspace_presentation,
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
