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

use openspec_core::{CacheEvent, DocumentChange};
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
/// Emitted when a document some surface is displaying changed on disk.
///
/// Distinct from [`EVENT_CACHE_UPDATED`] and every other name above, all of
/// which are derived from a [`CacheEvent`]. A document change mutates no cached
/// state and concerns no tree row: it comes from
/// [`openspec_core::document_watch`], travels its own channel, and is mapped by
/// [`document_envelope`] rather than by [`event_envelope`]. Expressing it as a
/// `CacheEvent` variant would have forced every existing consumer of that
/// stream, in three frontends, to grow an arm that ignores it.
pub const EVENT_DOCUMENT_CHANGED: &str = "document-changed";
/// Emitted by the desktop shell's View menu to toggle the sidebar's visibility.
/// Not a [`CacheEvent`]: the macOS menu item emits it directly, and only the
/// Tauri transport carries it (the web UI handles the same gesture with its own
/// keyboard binding — see the `spec-browser` capability). Carries no payload.
pub const EVENT_TOGGLE_SIDEBAR: &str = "toggle-sidebar";
/// Emitted by the desktop shell's View menu to toggle the commit rail's
/// visibility. Same transport story as [`EVENT_TOGGLE_SIDEBAR`].
pub const EVENT_TOGGLE_COMMIT_RAIL: &str = "toggle-commit-rail";

/// Identifies the document that changed: the browse root the reading surface
/// holds, and the document's path relative to it. Carries no content — the
/// surface re-reads through the guarded read, so exactly one code path reads a
/// file and exactly one guard applies to it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentChangedPayload {
    pub root: PathBuf,
    pub rel_path: String,
}

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
/// Map a document change to its `(name, payload)` wire form — the twin of
/// [`event_envelope`] for the document-watch channel. Both transports consume
/// this one mapping, so the desktop shell and the web SSE bridge emit
/// byte-identical frames.
pub fn document_envelope(change: &DocumentChange) -> (&'static str, Value) {
    (
        EVENT_DOCUMENT_CHANGED,
        to_value(DocumentChangedPayload {
            root: change.root.clone(),
            rel_path: change.rel_path.clone(),
        }),
    )
}

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
