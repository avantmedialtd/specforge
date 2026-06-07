mod commands;
#[cfg(target_os = "macos")]
mod dock_badge;
mod events;
#[cfg(target_os = "macos")]
mod menu;
mod notifications;
mod settings;
mod tray;
mod tray_icon;

use openspec_core::{
    build_backfill, change_lifecycle, current_season_index, task_completion_history, worktree_list,
    ActivityLog, CacheEvent, WatcherManager, WorkspacePresentationStore, WorkspaceRegistry,
};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tray_icon::{TrayGlyph, TrayGlyphState};

/// Bounded window for the one-time git backfill of historical achievements.
/// Matches the dashboard's full-year heatmap window so a year of contribution
/// cells has data to show on first launch.
const BACKFILL_SINCE: &str = "54 weeks ago";

/// Seed the activity log from git history on first launch (when the log is
/// empty). Once per distinct repository in the registry: change
/// creation/archival dates from `change_lifecycle`, and task completions from
/// the bounded `task_completion_history` diff. Keyed by the repo's main
/// worktree path. Non-git workspaces contribute nothing. Gated on an empty log
/// so it never re-runs the git scans on later launches.
fn backfill_activity(registry: &Arc<Mutex<WorkspaceRegistry>>, log: &Arc<ActivityLog>) {
    if !log.is_empty() {
        return;
    }
    let repo_ids = match registry.lock() {
        Ok(reg) => reg.repos(),
        Err(_) => return,
    };
    for repo_id in repo_ids {
        let main_wt = worktree_list(&repo_id)
            .into_iter()
            .find(|wt| wt.is_main)
            .map(|wt| wt.path)
            .or_else(|| repo_id.as_path().parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| repo_id.as_path().to_path_buf());
        let lifecycles = change_lifecycle(&repo_id);
        let task_history = task_completion_history(&repo_id, BACKFILL_SINCE);
        log.record_all(build_backfill(&main_wt, &lifecycles, &task_history));
    }
}

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
            }

            let config_dir = app
                .path()
                .app_config_dir()
                .expect("app config dir must be resolvable");
            std::fs::create_dir_all(&config_dir).ok();

            let workspaces_path = config_dir.join("workspaces.json");
            let settings_path = config_dir.join("settings.json");
            let presentation_path = config_dir.join("presentation.json");
            // The activity log lives alongside the other app-data stores —
            // never inside any workspace's `openspec/` tree — preserving the
            // Dashboard's read-only relationship to workspaces.
            let activity_path = config_dir.join("activity.json");

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

            // Seed the developer identity on first run from the git identities
            // detected across registered workspaces, so the profile and the
            // Dashboard's Me scope have a sensible default with no interaction.
            // Only when nothing is configured yet, and only the single most
            // obvious local identity is claimed — the user folds in any aliases
            // from Settings → Identity.
            if settings.snapshot().identity.aliases.is_empty() {
                let folders: Vec<std::path::PathBuf> = shared_registry
                    .lock()
                    .map(|r| r.entries().iter().map(|e| e.folder.uri.clone()).collect())
                    .unwrap_or_default();
                if let Some(primary) = openspec_core::detect_candidate_identities(&folders)
                    .into_iter()
                    .next()
                {
                    let _ = settings.set_identity(openspec_core::IdentityConfig {
                        display_name: primary.name.clone(),
                        aliases: vec![primary],
                    });
                }
            }

            // Seed the season rollover bookmark on first launch to the current
            // season, so the imminent git backfill (which reconstructs months of
            // history) does not fire a recap for every past month. A genuine
            // rollover later — the active season advancing past this bookmark —
            // surfaces exactly one recap, in `get_dashboard`.
            if settings.season_state().last_recapped_season_index.is_none() {
                let _ = settings.set_last_recapped_season(current_season_index());
            }

            let watcher = WatcherManager::with_registry(
                std::time::Duration::from_millis(200),
                Some(shared_registry.clone()),
            );

            // The append-only achievement log behind the Dashboard's progress
            // layer. Attached to the watcher so live re-parses record
            // task/artifact/change achievements; shared with `get_dashboard`
            // via managed state. Backfilled from git below (first launch only).
            let activity_log = Arc::new(ActivityLog::load(activity_path));
            watcher.set_activity_log(activity_log.clone());
            // Register as managed state immediately — before the git backfill
            // below — so the webview's first `get_dashboard` can never race
            // ahead of `.manage()` ("state not managed for field activityLog"
            // otherwise).
            app.manage(activity_log.clone());

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
            // `sync_repos` → `RepoMonitor::install`) must run inside an
            // active tokio context. Group them under one `block_on` rather
            // than calling some from sync code afterward, which would panic
            // with "there is no reactor running".
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
                // Initial aggregation so the first `get_workspace_views`
                // request returns a populated snapshot. After this seeding
                // call, every subsequent refresh of `last_views` happens
                // synchronously inside `handle_events` /
                // `RepoMonitor::reconcile` *before* the broadcast event that
                // triggered it reaches any subscriber — see the doc comment
                // on `WatcherManager::emit`.
                watcher_for_setup.aggregate_and_emit();
            });

            // Seed historical achievements from git so the progress layer is
            // populated on first launch rather than showing an empty board.
            // Runs once (gated on an empty log) on a background thread so the
            // bounded 90-day git scans never block setup or the first paint;
            // when done it emits a graph-changed nudge per repo so an open
            // Dashboard refetches the now-seeded log.
            {
                let registry_bf = shared_registry.clone();
                let log_bf = activity_log.clone();
                let watcher_bf = watcher.clone();
                std::thread::spawn(move || {
                    backfill_activity(&registry_bf, &log_bf);
                    let repos = registry_bf.lock().map(|r| r.repos()).unwrap_or_default();
                    for repo_id in repos {
                        watcher_bf.emit(CacheEvent::GraphChanged {
                            repo_id: repo_id.into_path_buf(),
                        });
                    }
                });
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
            commands::list_archived,
            commands::archived_artifact_status,
            commands::get_workspace_views,
            commands::get_active_count,
            commands::get_dashboard,
            commands::read_artifact,
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
            commands::set_workspace_presentation,
            commands::get_identity,
            commands::set_display_name,
            commands::set_identity_aliases,
            commands::set_equipped_treatment,
            commands::get_treatment_locker,
            commands::get_gamification_enabled,
            commands::set_gamification_enabled,
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
