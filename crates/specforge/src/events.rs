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
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}
