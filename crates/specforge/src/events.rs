//! Tauri event surface for cache changes.
//!
//! The frontend listens for these named events. Payload shapes are
//! serde-serialised structs declared here so the frontend can mirror the
//! types (and so the contract lives in one place).

use openspec_core::{CacheEvent, WatcherManager};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;

/// Emitted whenever a debounced batch of filesystem events caused the
/// cache for a workspace to be refreshed.
pub const EVENT_CACHE_UPDATED: &str = "cache-updated";

/// Emitted when a new active change directory appears in a workspace.
pub const EVENT_CHANGE_ADDED: &str = "change-added";

/// Emitted when an existing change directory moves into
/// `openspec/changes/archive/`.
pub const EVENT_CHANGE_ARCHIVED: &str = "change-archived";

/// Emitted when a tracked workspace was removed (a worktree disappeared
/// from `git worktree list` or was unregistered).
pub const EVENT_WORKSPACE_REMOVED: &str = "workspace-removed";

/// Emitted when a logical change first appears anywhere in a repository.
pub const EVENT_LOGICAL_CHANGE_ADDED: &str = "logical-change-added";

/// Emitted when every instance of a logical change is now archived.
pub const EVENT_LOGICAL_CHANGE_ARCHIVED: &str = "logical-change-archived";

/// Emitted when a new instance of a logical change appears.
pub const EVENT_INSTANCE_ADDED: &str = "instance-added";

/// Emitted when an instance of a logical change disappears.
pub const EVENT_INSTANCE_REMOVED: &str = "instance-removed";

/// Emitted after a successful `set_workspace_presentation` call so the
/// frontend can refetch the workspace list and re-render the tree's
/// top-level rows. Payload is empty — consumers refetch everything.
pub const EVENT_WORKSPACE_PRESENTATION_UPDATED: &str = "workspace-presentation-updated";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheUpdatedPayload {
    pub workspace: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeAddedPayload {
    pub workspace: PathBuf,
    pub change_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeArchivedPayload {
    pub workspace: PathBuf,
    pub change_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRemovedPayload {
    pub workspace: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalChangePayload {
    pub repo_id: PathBuf,
    pub change_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePayload {
    pub repo_id: PathBuf,
    pub change_name: String,
    pub worktree_path: PathBuf,
}

/// Subscribe to the watcher's `CacheEvent` stream and forward each variant
/// to the appropriate named Tauri event. Spawns a tokio task that lives as
/// long as the broadcast channel is open.
pub fn spawn_event_forwarder(app: AppHandle, watcher: &WatcherManager) {
    let mut rx = watcher.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(CacheEvent::Updated { workspace }) => {
                    let _ = app.emit(EVENT_CACHE_UPDATED, CacheUpdatedPayload { workspace });
                }
                Ok(CacheEvent::ChangeAdded {
                    workspace,
                    change_id,
                }) => {
                    let _ = app.emit(
                        EVENT_CHANGE_ADDED,
                        ChangeAddedPayload {
                            workspace,
                            change_id,
                        },
                    );
                }
                Ok(CacheEvent::ChangeArchived {
                    workspace,
                    change_id,
                }) => {
                    let _ = app.emit(
                        EVENT_CHANGE_ARCHIVED,
                        ChangeArchivedPayload {
                            workspace,
                            change_id,
                        },
                    );
                }
                Ok(CacheEvent::WorkspaceRemoved { workspace }) => {
                    let _ = app.emit(
                        EVENT_WORKSPACE_REMOVED,
                        WorkspaceRemovedPayload { workspace },
                    );
                }
                Ok(CacheEvent::LogicalChangeAdded {
                    repo_id,
                    change_name,
                }) => {
                    let _ = app.emit(
                        EVENT_LOGICAL_CHANGE_ADDED,
                        LogicalChangePayload {
                            repo_id,
                            change_name,
                        },
                    );
                }
                Ok(CacheEvent::LogicalChangeArchived {
                    repo_id,
                    change_name,
                }) => {
                    let _ = app.emit(
                        EVENT_LOGICAL_CHANGE_ARCHIVED,
                        LogicalChangePayload {
                            repo_id,
                            change_name,
                        },
                    );
                }
                Ok(CacheEvent::InstanceAdded {
                    repo_id,
                    change_name,
                    worktree_path,
                }) => {
                    let _ = app.emit(
                        EVENT_INSTANCE_ADDED,
                        InstancePayload {
                            repo_id,
                            change_name,
                            worktree_path,
                        },
                    );
                }
                Ok(CacheEvent::InstanceRemoved {
                    repo_id,
                    change_name,
                    worktree_path,
                }) => {
                    let _ = app.emit(
                        EVENT_INSTANCE_REMOVED,
                        InstancePayload {
                            repo_id,
                            change_name,
                            worktree_path,
                        },
                    );
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}
