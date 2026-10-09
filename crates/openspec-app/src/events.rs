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
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::pull_request_detail::PullRequestReference;
use crate::settings::PanelPosition;

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
/// Emitted when the opt-in BitBucket pull-request snapshot changed. Carries no
/// payload — the frontend re-reads the snapshot via
/// `get_bitbucket_pull_requests`. Derived from
/// [`CacheEvent::BitbucketPullRequestsUpdated`], which the BitBucket poller
/// (`crate::bitbucket`) raises; a background thread has no transport in hand,
/// so it announces through the cache stream exactly as the quota pollers do,
/// and both transports pick it up through [`event_envelope`]. The name carries
/// the provider so it is never taken for the GitHub panel's announcement
/// (`bitbucket-pull-requests`: *The Snapshot Is Announced on the Cache Stream*).
pub const EVENT_BITBUCKET_PULL_REQUESTS_UPDATED: &str = "bitbucket-pull-requests-updated";
/// Emitted when the opt-in GitHub pull-request snapshot changed. Carries no
/// payload — the frontend re-reads the snapshot via `get_github_pull_requests`.
/// Derived from [`CacheEvent::GithubPullRequestsUpdated`], which the GitHub
/// poller (`crate::github`) raises, for the same reason as its BitBucket twin.
pub const EVENT_GITHUB_PULL_REQUESTS_UPDATED: &str = "github-pull-requests-updated";
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
/// Emitted by the desktop shell's application menu when the user chooses
/// *Settings…* (Cmd+,), asking the main window to show the Settings view
/// (`application-menu`: *Settings Menu Item*). Same transport story as
/// [`EVENT_TOGGLE_SIDEBAR`]: the menu item emits it directly, only the Tauri
/// transport carries it (in a browser, Cmd+, belongs to the browser's own
/// preferences), and it carries no payload.
pub const EVENT_OPEN_SETTINGS: &str = "open-settings";
/// Emitted after a successful `set_document_width` so every open window adopts
/// the new reading width without being reopened. Carries the new value, so a
/// listener re-stamps directly rather than making a round trip to read back
/// what it was just told.
///
/// Not derived from a [`CacheEvent`] — the command (or web dispatch) emits it
/// directly, for the reason [`EVENT_DOCUMENT_CHANGED`] gives at length: a
/// `CacheEvent` variant would force every existing consumer of that stream, in
/// three frontends, to grow an arm that ignores it. Unlike the two toggles
/// above, this one travels BOTH transports — the browser skin renders the same
/// documents and honours the same preference.
pub const EVENT_DOCUMENT_WIDTH_CHANGED: &str = "document-width-changed";
/// Emitted after a successful `set_bitbucket_panel_position` or
/// `set_github_panel_position` so every open window — and every connected
/// browser skin — re-seats that provider's pull-request panel without being
/// reopened. Carries [`PanelMovedPayload`], so a listener moves the named panel
/// directly rather than reading back what it was just told.
///
/// Not derived from a [`CacheEvent`], for the reason
/// [`EVENT_DOCUMENT_WIDTH_CHANGED`] gives: it is raised by a command, which has
/// the transport in hand, so the command (or web dispatch) emits it directly —
/// on both transports, since the browser skin renders the same panel.
pub const EVENT_PULL_REQUEST_PANEL_MOVED: &str = "pull-request-panel-moved";
/// Emitted after a successful `set_commit_history_enabled` so every open main
/// window — and every connected browser skin — adopts the Commit history switch
/// without being reopened (`commit-graph`: *Commit History Can Be Turned Off*).
/// Carries the new value as a bare boolean, so a listener applies it directly
/// rather than reading back what it was just told.
///
/// Not derived from a [`CacheEvent`], for the reason
/// [`EVENT_DOCUMENT_WIDTH_CHANGED`] gives: it is raised by a command, which has
/// the transport in hand, so the command (or web dispatch) emits it directly —
/// on both transports, since the browser skin hosts the same rail.
pub const EVENT_COMMIT_HISTORY_ENABLED_CHANGED: &str = "commit-history-enabled-changed";
/// Emitted after a stored review mark or unmark, carrying the pull request's
/// [`PullRequestReference`], so every view showing that pull request re-reads
/// its progress (`pull-request-viewer`: *Review Progress*).
///
/// A [`ServiceNotice`], never a [`CacheEvent`] and never a command's direct
/// emit: the service raises it on its own broadcast, so it reaches every
/// window and every served tab of that service, whichever transport set the
/// mark. A direct emit would reach only its own transport, and a `CacheEvent`
/// variant would reach every exhaustive consumer of the cache stream — the
/// notifications, the tray, the Dock badge and the terminal (design D9).
pub const EVENT_REVIEW_PROGRESS_CHANGED: &str = "review-progress-changed";
/// Emitted whenever a pull-request provider's enabled flag is set, carrying
/// [`PullRequestProviderChangedPayload`], so every root that resolves
/// pull-request addresses keeps the flag current (`pull-request-viewer`:
/// *Provider Enabled Flags Stay Current*). A [`ServiceNotice`], for the reason
/// [`EVENT_REVIEW_PROGRESS_CHANGED`] gives.
pub const EVENT_PULL_REQUEST_PROVIDER_CHANGED: &str = "pull-request-provider-changed";
/// Emitted after `set_review_skip_patterns` stores a list, the empty one
/// included, carrying [`ReviewSkipPatternsChangedPayload`], so every
/// pull-request view reads its review progress again and an open Settings view
/// shows the list now stored (`pull-request-viewer`: *Review Skip Patterns*).
///
/// Not derived from a [`CacheEvent`] and not a [`ServiceNotice`]: the setting
/// command emits it directly, on both transports, as
/// [`EVENT_COMMIT_HISTORY_ENABLED_CHANGED`] is (`review-skip-patterns` design
/// D9). It is not [`EVENT_REVIEW_PROGRESS_CHANGED`] either, whose reference
/// names one pull request while a rule change touches every one.
pub const EVENT_REVIEW_SKIP_PATTERNS_CHANGED: &str = "review-skip-patterns-changed";

/// Which pull-request provider a panel — or a panel event — belongs to. The
/// two panels are independent twins (`github-pull-requests`: *Opt-in GitHub
/// Pull-Request Tracking*), so one event carries the name of the panel it moves.
/// Deserialized too, as part of the pull-request reference the frontend sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PullRequestProvider {
    Bitbucket,
    Github,
}

/// The payload of [`EVENT_PULL_REQUEST_PROVIDER_CHANGED`]: which provider, and
/// its enabled flag as just set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestProviderChangedPayload {
    pub provider: PullRequestProvider,
    pub enabled: bool,
}

/// A notice the service raises on its own broadcast
/// (`AppService::subscribe_notices`), for state no `CacheEvent` describes. Each
/// transport drains that broadcast through [`notice_envelope`], so every window
/// and every served tab of one service hears it, whichever transport caused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceNotice {
    /// A pull request's stored review progress changed.
    ReviewProgressChanged(PullRequestReference),
    /// A provider's enabled flag was set.
    PullRequestProviderChanged(PullRequestProviderChangedPayload),
}

/// The payload of [`EVENT_REVIEW_SKIP_PATTERNS_CHANGED`]: the skip patterns now
/// stored. A list just stored has been accepted, so no pattern of it is in
/// error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSkipPatternsChangedPayload {
    pub patterns: Vec<String>,
}

/// The payload of [`EVENT_PULL_REQUEST_PANEL_MOVED`]: which panel moved, and
/// its new slot.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelMovedPayload {
    pub provider: PullRequestProvider,
    pub position: PanelPosition,
}

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

/// Map a service notice to its `(name, payload)` wire form — the twin of
/// [`document_envelope`] for the service's notice broadcast. Both transports
/// consume this one mapping, so a window and a served tab hear the same frame.
pub fn notice_envelope(notice: &ServiceNotice) -> (&'static str, Value) {
    match notice {
        ServiceNotice::ReviewProgressChanged(reference) => {
            (EVENT_REVIEW_PROGRESS_CHANGED, to_value(reference))
        }
        ServiceNotice::PullRequestProviderChanged(payload) => {
            (EVENT_PULL_REQUEST_PROVIDER_CHANGED, to_value(payload))
        }
    }
}

/// Map a [`CacheEvent`] to its `(event name, JSON payload)` wire form. The one
/// mapping both event transports share, so a Tauri `app.emit` and an SSE frame
/// carry identical names and payloads for the same event.
///
/// Payload-less events (`QuotaUpdated` and the two pull-request variants) map to
/// [`Value::Null`]; the frontend ignores the body and re-reads via a command.
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
        CacheEvent::BitbucketPullRequestsUpdated => {
            (EVENT_BITBUCKET_PULL_REQUESTS_UPDATED, Value::Null)
        }
        CacheEvent::GithubPullRequestsUpdated => (EVENT_GITHUB_PULL_REQUESTS_UPDATED, Value::Null),
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

    /// The wire contract the frontend re-declares by hand in `src/types.ts`:
    /// the event name, and a `relPath` key that must survive the camelCase
    /// rename. There is no codegen, so this test is the only thing holding the
    /// two sides together.
    #[test]
    fn document_changed_carries_camelcase_rel_path() {
        let (name, payload) = document_envelope(&DocumentChange {
            root: PathBuf::from("/ws"),
            rel_path: "openspec/specs/web-ui/spec.md".into(),
        });
        assert_eq!(name, "document-changed");
        assert_eq!(payload["root"], "/ws");
        assert_eq!(payload["relPath"], "openspec/specs/web-ui/spec.md");
        // Identifiers only: a surface re-reads through the guarded read, so a
        // content key here would mean a second path that serves file bytes.
        assert!(payload.get("body").is_none());
        assert!(payload.get("content").is_none());
    }

    /// Every name a [`CacheEvent`] maps to.
    const CACHE_EVENT_NAMES: [&str; 12] = [
        EVENT_CACHE_UPDATED,
        EVENT_CHANGE_ADDED,
        EVENT_CHANGE_ARCHIVED,
        EVENT_WORKSPACE_REMOVED,
        EVENT_LOGICAL_CHANGE_ADDED,
        EVENT_LOGICAL_CHANGE_ARCHIVED,
        EVENT_INSTANCE_ADDED,
        EVENT_INSTANCE_REMOVED,
        EVENT_GRAPH_CHANGED,
        EVENT_QUOTA_UPDATED,
        EVENT_BITBUCKET_PULL_REQUESTS_UPDATED,
        EVENT_GITHUB_PULL_REQUESTS_UPDATED,
    ];

    /// A document change must not be mistaken for a cache event: they travel
    /// separate channels and every existing consumer of the cache stream is
    /// meant to be untouched by this name.
    #[test]
    fn the_document_event_name_is_distinct_from_every_cache_event_name() {
        let cache_names = CACHE_EVENT_NAMES;
        assert!(!cache_names.contains(&EVENT_DOCUMENT_CHANGED));
        // The panel-move event is a command's direct emit, like the reading
        // width: a consumer that took it for the snapshot announcement would
        // re-fetch the list on every move.
        assert!(!cache_names.contains(&EVENT_PULL_REQUEST_PANEL_MOVED));
        // Same contract for the reading-width event, and it must not collide
        // with the document event it sits beside either — one re-reads a file,
        // the other re-stamps an attribute, and a consumer that confused them
        // would refetch every open document on a settings change.
        assert!(!cache_names.contains(&EVENT_DOCUMENT_WIDTH_CHANGED));
        assert_ne!(EVENT_DOCUMENT_WIDTH_CHANGED, EVENT_DOCUMENT_CHANGED);
        // And for the Commit history switch: a consumer that took it for
        // `graph-changed` would re-fetch a graph the reader just turned off.
        assert!(!cache_names.contains(&EVENT_COMMIT_HISTORY_ENABLED_CHANGED));
        // And for the menu's Settings… request, which re-reads nothing.
        assert!(!cache_names.contains(&EVENT_OPEN_SETTINGS));
    }

    /// The Settings… request's name is the literal `src/types.ts` mirrors by
    /// hand, and it must not be either pane toggle the same menu emits: a
    /// listener that confused them would flip a pane when the user asked for
    /// Settings, or open Settings from Cmd+B.
    #[test]
    fn the_open_settings_event_is_its_own_name() {
        assert_eq!(EVENT_OPEN_SETTINGS, "open-settings");
        assert_ne!(EVENT_OPEN_SETTINGS, EVENT_TOGGLE_SIDEBAR);
        assert_ne!(EVENT_OPEN_SETTINGS, EVENT_TOGGLE_COMMIT_RAIL);
    }

    /// The switch's name is the literal `src/types.ts` mirrors by hand, and it
    /// must not be the rail's visibility toggle it sits beside: a listener that
    /// confused the two would flip the rail's per-surface visibility on a
    /// settings change, or change a setting from a keyboard shortcut.
    #[test]
    fn commit_history_event_is_its_own_name_beside_the_rail_toggle() {
        assert_eq!(
            EVENT_COMMIT_HISTORY_ENABLED_CHANGED,
            "commit-history-enabled-changed"
        );
        assert_ne!(
            EVENT_COMMIT_HISTORY_ENABLED_CHANGED,
            EVENT_TOGGLE_COMMIT_RAIL
        );
        assert_ne!(
            EVENT_COMMIT_HISTORY_ENABLED_CHANGED,
            EVENT_DOCUMENT_WIDTH_CHANGED
        );
    }

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

    /// Each provider has its own name, and neither is `quota-updated`: reusing
    /// one would make the quota pills or the other panel re-fetch on every
    /// refresh. The provider-less `pull-requests-updated` is retired, so no
    /// listener can mistake one provider's announcement for the other's
    /// (`bitbucket-pull-requests`: *The Snapshot Is Announced on the Cache
    /// Stream*).
    #[test]
    fn each_pull_request_provider_has_its_own_name_and_a_null_payload() {
        let (bitbucket, payload) = event_envelope(&CacheEvent::BitbucketPullRequestsUpdated);
        assert_eq!(bitbucket, "bitbucket-pull-requests-updated");
        assert!(payload.is_null());
        let (github, payload) = event_envelope(&CacheEvent::GithubPullRequestsUpdated);
        assert_eq!(github, "github-pull-requests-updated");
        assert!(payload.is_null());
        for name in [bitbucket, github] {
            assert_ne!(name, EVENT_QUOTA_UPDATED);
            assert_ne!(name, "pull-requests-updated");
        }
        assert_ne!(bitbucket, github);
    }

    /// Each notice is its own name: not a cache event's, which every consumer
    /// of the cache stream would act on, not the document event's or the
    /// panel-moved event's, and not the other notice's. `src/types.ts` mirrors
    /// both literals by hand.
    #[test]
    fn the_notice_names_are_distinct_from_every_cache_and_document_event_name() {
        assert_eq!(EVENT_REVIEW_PROGRESS_CHANGED, "review-progress-changed");
        assert_eq!(
            EVENT_PULL_REQUEST_PROVIDER_CHANGED,
            "pull-request-provider-changed"
        );
        for notice in [
            EVENT_REVIEW_PROGRESS_CHANGED,
            EVENT_PULL_REQUEST_PROVIDER_CHANGED,
        ] {
            assert!(!CACHE_EVENT_NAMES.contains(&notice), "{notice}");
            assert_ne!(notice, EVENT_DOCUMENT_CHANGED);
            assert_ne!(notice, EVENT_PULL_REQUEST_PANEL_MOVED);
        }
        assert_ne!(
            EVENT_REVIEW_PROGRESS_CHANGED,
            EVENT_PULL_REQUEST_PROVIDER_CHANGED
        );
    }

    /// The wire contract `src/types.ts` re-declares by hand: each notice's
    /// name and its camelCase payload, the reference itself for review
    /// progress, spelt as it was raised.
    #[test]
    fn each_notice_maps_to_its_name_and_its_camel_case_payload() {
        let reference = PullRequestReference {
            provider: PullRequestProvider::Github,
            owner: "Acme".to_string(),
            repo: "api".to_string(),
            number: 42,
        };
        let (name, payload) = notice_envelope(&ServiceNotice::ReviewProgressChanged(reference));
        assert_eq!(name, EVENT_REVIEW_PROGRESS_CHANGED);
        assert_eq!(
            payload,
            serde_json::json!({ "provider": "github", "owner": "Acme", "repo": "api", "number": 42 })
        );

        let (name, payload) = notice_envelope(&ServiceNotice::PullRequestProviderChanged(
            PullRequestProviderChangedPayload {
                provider: PullRequestProvider::Bitbucket,
                enabled: false,
            },
        ));
        assert_eq!(name, EVENT_PULL_REQUEST_PROVIDER_CHANGED);
        assert_eq!(
            payload,
            serde_json::json!({ "provider": "bitbucket", "enabled": false })
        );
    }

    /// The skip patterns' event is the literal `src/types.ts` mirrors by hand,
    /// and a command's direct emit: no cache event's name, which every
    /// consumer of the cache stream would act on, no notice's, and not review
    /// progress's own, which names one pull request.
    #[test]
    fn the_skip_patterns_event_is_its_own_name_and_no_cache_event() {
        assert_eq!(
            EVENT_REVIEW_SKIP_PATTERNS_CHANGED,
            "review-skip-patterns-changed"
        );
        assert!(!CACHE_EVENT_NAMES.contains(&EVENT_REVIEW_SKIP_PATTERNS_CHANGED));
        for other in [
            EVENT_REVIEW_PROGRESS_CHANGED,
            EVENT_PULL_REQUEST_PROVIDER_CHANGED,
            EVENT_COMMIT_HISTORY_ENABLED_CHANGED,
            EVENT_DOCUMENT_CHANGED,
            EVENT_PULL_REQUEST_PANEL_MOVED,
        ] {
            assert_ne!(EVENT_REVIEW_SKIP_PATTERNS_CHANGED, other);
        }
    }

    /// The payload `src/types.ts` re-declares by hand: the list now stored,
    /// and nothing else, the empty list included.
    #[test]
    fn the_skip_patterns_payload_carries_the_list_now_stored() {
        let payload = to_value(ReviewSkipPatternsChangedPayload {
            patterns: vec!["**/tests/**".to_string(), "/^docs//".to_string()],
        });
        assert_eq!(
            payload,
            serde_json::json!({ "patterns": ["**/tests/**", "/^docs//"] })
        );
        let emptied = to_value(ReviewSkipPatternsChangedPayload {
            patterns: Vec::new(),
        });
        assert_eq!(emptied, serde_json::json!({ "patterns": [] }));
    }

    /// The wire contract `src/types.ts` re-declares by hand: the event name, a
    /// camelCase `provider`, and a `position` key carrying the kebab-case slot.
    #[test]
    fn panel_moved_carries_the_provider_and_the_kebab_case_position() {
        assert_eq!(EVENT_PULL_REQUEST_PANEL_MOVED, "pull-request-panel-moved");
        let payload = to_value(PanelMovedPayload {
            provider: PullRequestProvider::Bitbucket,
            position: PanelPosition::RightTop,
        });
        assert_eq!(
            payload,
            serde_json::json!({ "provider": "bitbucket", "position": "right-top" })
        );
        let payload = to_value(PanelMovedPayload {
            provider: PullRequestProvider::Github,
            position: PanelPosition::LeftBottom,
        });
        assert_eq!(
            payload,
            serde_json::json!({ "provider": "github", "position": "left-bottom" })
        );
    }
}
