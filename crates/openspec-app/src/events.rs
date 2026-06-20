//! The shared cache-event surface: named events + payload shapes + the mapping
//! from a [`CacheEvent`] to its `(name, payload)` wire form.
//!
//! Both user-facing event transports consume this one mapping so they emit
//! byte-identical wire shapes:
//!
//! - the Tauri shell's forwarder (`specforge/events.rs`) maps each event to an
//!   `app.emit(name, payload)`;
//! - the web server's SSE bridge (`specforge-web`) maps each event to an
//!   `text/event-stream` frame.
//!
//! Previously the names and payload structs lived in the Tauri crate, which
//! meant the web bridge would have had to duplicate them (and could drift). They
//! live here, above both frontends, so the contract has a single source.

use openspec_core::CacheEvent;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

/// Emitted whenever a debounced batch of filesystem events caused the cache for
/// a workspace to be refreshed.
pub const EVENT_CACHE_UPDATED: &str = "cache-updated";
/// Emitted when a new active change directory appears in a workspace.
pub const EVENT_CHANGE_ADDED: &str = "change-added";
/// Emitted when an existing change directory moves into
/// `openspec/changes/archive/`.
pub const EVENT_CHANGE_ARCHIVED: &str = "change-archived";
/// Emitted when a tracked workspace was removed (a worktree disappeared from
/// `git worktree list` or was unregistered).
pub const EVENT_WORKSPACE_REMOVED: &str = "workspace-removed";
/// Emitted when a logical change first appears anywhere in a repository.
pub const EVENT_LOGICAL_CHANGE_ADDED: &str = "logical-change-added";
/// Emitted when every instance of a logical change is now archived.
pub const EVENT_LOGICAL_CHANGE_ARCHIVED: &str = "logical-change-archived";
/// Emitted when a new instance of a logical change appears.
pub const EVENT_INSTANCE_ADDED: &str = "instance-added";
/// Emitted when an instance of a logical change disappears.
pub const EVENT_INSTANCE_REMOVED: &str = "instance-removed";
/// Emitted after a successful `set_workspace_presentation` so the frontend
/// refetches the workspace list. Not derived from a [`CacheEvent`] — the
/// command (or web dispatch) emits it directly. Carries no payload.
pub const EVENT_WORKSPACE_PRESENTATION_UPDATED: &str = "workspace-presentation-updated";
/// Emitted when a repository's refs move (new commit, branch/tag change, HEAD
/// movement). The commit-graph rail re-fetches the affected repo's graph.
pub const EVENT_GRAPH_CHANGED: &str = "graph-changed";
/// Emitted when the opt-in Claude usage-quota snapshot is refreshed. Carries no
/// payload — the frontend re-reads the snapshot via `get_claude_quota`.
pub const EVENT_QUOTA_UPDATED: &str = "quota-updated";

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphChangedPayload {
    pub repo_id: PathBuf,
}

/// Map a [`CacheEvent`] to its `(event name, JSON payload)` wire form. The one
/// mapping both event transports share, so a Tauri `app.emit` and an SSE frame
/// carry identical names and payloads for the same event.
///
/// Payload-less events (`QuotaUpdated`) map to [`Value::Null`]; the frontend
/// ignores the body and re-reads via a command.
pub fn event_envelope(event: &CacheEvent) -> (&'static str, Value) {
    match event {
        CacheEvent::Updated { workspace } => (
            EVENT_CACHE_UPDATED,
            to_value(CacheUpdatedPayload {
                workspace: workspace.clone(),
            }),
        ),
        CacheEvent::ChangeAdded {
            workspace,
            change_id,
        } => (
            EVENT_CHANGE_ADDED,
            to_value(ChangeAddedPayload {
                workspace: workspace.clone(),
                change_id: change_id.clone(),
            }),
        ),
        CacheEvent::ChangeArchived {
            workspace,
            change_id,
        } => (
            EVENT_CHANGE_ARCHIVED,
            to_value(ChangeArchivedPayload {
                workspace: workspace.clone(),
                change_id: change_id.clone(),
            }),
        ),
        CacheEvent::WorkspaceRemoved { workspace } => (
            EVENT_WORKSPACE_REMOVED,
            to_value(WorkspaceRemovedPayload {
                workspace: workspace.clone(),
            }),
        ),
        CacheEvent::LogicalChangeAdded {
            repo_id,
            change_name,
        } => (
            EVENT_LOGICAL_CHANGE_ADDED,
            to_value(LogicalChangePayload {
                repo_id: repo_id.clone(),
                change_name: change_name.clone(),
            }),
        ),
        CacheEvent::LogicalChangeArchived {
            repo_id,
            change_name,
        } => (
            EVENT_LOGICAL_CHANGE_ARCHIVED,
            to_value(LogicalChangePayload {
                repo_id: repo_id.clone(),
                change_name: change_name.clone(),
            }),
        ),
        CacheEvent::InstanceAdded {
            repo_id,
            change_name,
            worktree_path,
        } => (
            EVENT_INSTANCE_ADDED,
            to_value(InstancePayload {
                repo_id: repo_id.clone(),
                change_name: change_name.clone(),
                worktree_path: worktree_path.clone(),
            }),
        ),
        CacheEvent::InstanceRemoved {
            repo_id,
            change_name,
            worktree_path,
        } => (
            EVENT_INSTANCE_REMOVED,
            to_value(InstancePayload {
                repo_id: repo_id.clone(),
                change_name: change_name.clone(),
                worktree_path: worktree_path.clone(),
            }),
        ),
        CacheEvent::GraphChanged { repo_id } => (
            EVENT_GRAPH_CHANGED,
            to_value(GraphChangedPayload {
                repo_id: repo_id.clone(),
            }),
        ),
        CacheEvent::QuotaUpdated => (EVENT_QUOTA_UPDATED, Value::Null),
    }
}

/// Serialize a payload struct to a JSON value. The payloads are plain structs of
/// strings/paths, so serialization is infallible; fall back to `Null` rather
/// than panic on the impossible error.
fn to_value<T: Serialize>(payload: T) -> Value {
    serde_json::to_value(payload).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updated_maps_to_camelcase_workspace() {
        let (name, payload) = event_envelope(&CacheEvent::Updated {
            workspace: PathBuf::from("/ws"),
        });
        assert_eq!(name, "cache-updated");
        assert_eq!(payload["workspace"], "/ws");
    }

    #[test]
    fn instance_added_carries_camelcase_worktree_path() {
        let (name, payload) = event_envelope(&CacheEvent::InstanceAdded {
            repo_id: PathBuf::from("/r/.git"),
            change_name: "add-web-ui".into(),
            worktree_path: PathBuf::from("/r/wt"),
        });
        assert_eq!(name, "instance-added");
        assert_eq!(payload["repoId"], "/r/.git");
        assert_eq!(payload["changeName"], "add-web-ui");
        assert_eq!(payload["worktreePath"], "/r/wt");
    }

    #[test]
    fn quota_updated_has_null_payload() {
        let (name, payload) = event_envelope(&CacheEvent::QuotaUpdated);
        assert_eq!(name, "quota-updated");
        assert!(payload.is_null());
    }
}
