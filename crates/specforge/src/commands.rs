//! `#[tauri::command]` handlers exposed to the frontend.
//!
//! Each handler converts errors to `String` since Tauri serialises the
//! return type to JSON for the JS side. Most handlers borrow shared state
//! via `State<'_, T>`; async handlers release any `std::sync::Mutex`
//! guards before crossing `await` boundaries.

use crate::settings::SettingsStore;
use openspec_core::{
    ChangeData, RegisteredWorkspace, WatcherManager, WorkspaceRegistry, WorkspaceView,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri_plugin_autostart::ManagerExt;

type SharedRegistry = Arc<Mutex<WorkspaceRegistry>>;
type SharedSettings = Arc<SettingsStore>;

#[tauri::command]
pub async fn register_workspace(
    path: String,
    registry: State<'_, SharedRegistry>,
    watcher: State<'_, WatcherManager>,
) -> Result<RegisteredWorkspace, String> {
    // `register` returns the user-registered entry plus any auto-discovered
    // sibling worktrees of the same git repo. The first element is always
    // the user-registered folder.
    let added = {
        let mut reg = registry.lock().map_err(|e| e.to_string())?;
        reg.register(PathBuf::from(path))
            .map_err(|e| e.to_string())?
    };

    let primary = added
        .first()
        .cloned()
        .ok_or_else(|| "register returned no folders".to_string())?;

    // Start watchers for every newly-tracked workspace (the user-registered
    // one and any discovered siblings).
    for folder in &added {
        if folder.uri.is_dir() {
            if let Err(e) = watcher.add_workspace(folder.clone()).await {
                eprintln!("failed to add watcher for {}: {e}", folder.uri.display());
            }
        }
    }

    // Install (or update) per-repo monitors so future runtime worktree
    // adds/removes for this repo are picked up automatically.
    watcher.sync_repos();

    Ok(RegisteredWorkspace {
        uri: primary.uri,
        name: primary.name,
        is_missing: false,
    })
}

#[tauri::command]
pub fn unregister_workspace(
    path: String,
    registry: State<'_, SharedRegistry>,
    watcher: State<'_, WatcherManager>,
) -> Result<bool, String> {
    let path_buf = PathBuf::from(path);
    let removed = registry
        .lock()
        .map_err(|e| e.to_string())?
        .unregister(&path_buf)
        .map_err(|e| e.to_string())?;

    // Tear down watchers for every removed path (the user-registered one
    // plus any cascaded discovered worktrees of the same repo).
    let any_removed = !removed.is_empty();
    for p in &removed {
        watcher.remove_workspace(p);
    }
    // Drop any repo monitors whose repo no longer has tracked workspaces.
    watcher.sync_repos();
    Ok(any_removed)
}

#[tauri::command]
pub fn list_workspaces(
    registry: State<'_, SharedRegistry>,
) -> Result<Vec<RegisteredWorkspace>, String> {
    Ok(registry.lock().map_err(|e| e.to_string())?.list())
}

#[tauri::command]
pub fn get_changes(
    workspace: String,
    watcher: State<'_, WatcherManager>,
) -> Result<Vec<ChangeData>, String> {
    Ok(watcher.changes_for(&PathBuf::from(workspace)))
}

/// Returns one entry per tracked top-level workspace: either an aggregated
/// [`WorkspaceView::Repo`] grouping all worktrees of a git repository, or
/// a [`WorkspaceView::Flat`] for a non-git workspace. This is the command
/// the new repo/instance-aware tree consumes.
#[tauri::command]
pub fn get_workspace_views(
    watcher: State<'_, WatcherManager>,
) -> Result<Vec<WorkspaceView>, String> {
    Ok(watcher.workspace_views())
}

#[tauri::command]
pub fn get_active_count(watcher: State<'_, WatcherManager>) -> Result<usize, String> {
    // Counts non-archived logical changes across every tracked entry. A
    // logical change touched by multiple worktrees contributes 1.
    Ok(watcher.total_active_logical_count())
}

/// Returns the raw markdown for one artifact of a change.
///
/// `artifact_kind` must be one of `"proposal"`, `"design"`, `"tasks"`, or
/// `"spec"`. The `capability` argument is required (and only used) when
/// kind is `"spec"`.
///
/// A path-traversal guard canonicalises the resolved file and rejects
/// anything outside the workspace's `openspec/changes/` subtree.
#[tauri::command]
pub async fn read_artifact(
    workspace: String,
    change_id: String,
    artifact_kind: String,
    capability: Option<String>,
) -> Result<String, String> {
    let workspace_path = PathBuf::from(workspace);
    let changes_root = workspace_path.join("openspec").join("changes");
    let change_dir = changes_root.join(&change_id);

    let file_path = match artifact_kind.as_str() {
        "proposal" => change_dir.join("proposal.md"),
        "design" => change_dir.join("design.md"),
        "tasks" => change_dir.join("tasks.md"),
        "spec" => {
            let cap = capability
                .ok_or_else(|| "spec artifact requires a `capability` name".to_string())?;
            change_dir.join("specs").join(cap).join("spec.md")
        }
        other => return Err(format!("unknown artifact kind: {other}")),
    };

    let changes_root_canonical = changes_root
        .canonicalize()
        .map_err(|e| format!("workspace changes directory missing: {e}"))?;
    let resolved = file_path
        .canonicalize()
        .map_err(|e| format!("artifact not found: {e}"))?;
    if !resolved.starts_with(&changes_root_canonical) {
        return Err("artifact path escapes workspace".to_string());
    }

    tokio::fs::read_to_string(&resolved)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_launch_on_login(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_launch_on_login(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_notifications_enabled(settings: State<'_, SharedSettings>) -> Result<bool, String> {
    Ok(settings.snapshot().notifications_enabled)
}

#[tauri::command]
pub fn set_notifications_enabled(
    enabled: bool,
    settings: State<'_, SharedSettings>,
) -> Result<(), String> {
    settings
        .set_notifications_enabled(enabled)
        .map_err(|e| e.to_string())
}
