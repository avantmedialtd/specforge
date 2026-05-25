//! Desktop-notification dispatcher.
//!
//! Subscribes to [`openspec_core::CacheEvent`] and fires a desktop
//! notification only on structural transitions — new active change or
//! a change moving into `archive/`. Plain `Updated` events (file edits
//! within existing changes) never notify, per spec.
//!
//! The dispatcher honours the in-app notifications-enabled toggle. If
//! the user turns notifications off in settings, events still flow but
//! no OS notification is shown.

use crate::settings::SettingsStore;
use openspec_core::{CacheEvent, WatcherManager};
use std::path::Path;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::broadcast;

/// Spawn the dispatcher task. Returns immediately.
pub fn spawn_notification_dispatcher(
    app: AppHandle,
    watcher: &WatcherManager,
    settings: Arc<SettingsStore>,
) {
    let mut rx = watcher.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => dispatch(&app, &settings, event),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

fn dispatch(app: &AppHandle, settings: &SettingsStore, event: CacheEvent) {
    // Notifications are scoped to *logical* transitions only — a new change
    // appearing anywhere in a repository, or every instance of a change
    // being archived. Per-instance churn (the harness opening or closing
    // ephemeral worktrees) is intentionally silent.
    let (title, body) = match event {
        CacheEvent::LogicalChangeAdded {
            repo_id,
            change_name,
        } => (
            "New change",
            format!("{} · {}", display_name(&repo_id), change_name),
        ),
        CacheEvent::LogicalChangeArchived {
            repo_id,
            change_name,
        } => (
            "Change archived",
            format!("{} · {}", display_name(&repo_id), change_name),
        ),
        // Non-git workspaces still emit the original instance-grained events;
        // forward those so users with workspaces outside any git repo continue
        // to get notifications.
        CacheEvent::ChangeAdded {
            workspace,
            change_id,
        } => (
            "New change",
            format!("{} · {}", display_name(&workspace), change_id),
        ),
        CacheEvent::ChangeArchived {
            workspace,
            change_id,
        } => (
            "Change archived",
            format!("{} · {}", display_name(&workspace), change_id),
        ),
        // All other events are silent by design.
        CacheEvent::Updated { .. }
        | CacheEvent::WorkspaceRemoved { .. }
        | CacheEvent::InstanceAdded { .. }
        | CacheEvent::InstanceRemoved { .. } => return,
    };

    // Check setting after matching so the lock is held only briefly and
    // never across the OS notification call.
    if !settings.snapshot().notifications_enabled {
        return;
    }

    let _ = app.notification().builder().title(title).body(&body).show();
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string()
}
