//! Tauri event surface for cache changes.
//!
//! The frontend listens for these named events. Their *names* and *payload
//! shapes* now live in `openspec_app::events` (above both frontends) so the web
//! server's SSE bridge reproduces them identically — this module re-exports them
//! (so existing call sites keep working) and owns only the Tauri-specific
//! forwarding sinks, which map each `CacheEvent`, document change and service
//! notice through the shared envelopes and emit it as a Tauri event.

use openspec_app::{document_envelope, event_envelope, notice_envelope, AppService};
use openspec_core::{DocumentWatcher, WatcherManager};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast;

// The presentation-updated event is emitted directly by a command (not via the
// `CacheEvent` forwarder), so this crate still references it by name. The other
// event names now live in `openspec_app::events` and are consumed there by
// `event_envelope`; re-export only what this crate uses to avoid dead re-exports.
pub use openspec_app::events::EVENT_WORKSPACE_PRESENTATION_UPDATED;

// Likewise the reading-width event, emitted directly by `set_document_width`.
// Unlike the pane toggles below it is not macOS-only: every host renders the
// same documents at the same configured width.
pub use openspec_app::events::EVENT_DOCUMENT_WIDTH_CHANGED;

// The Commit history switch's event, emitted directly by
// `set_commit_history_enabled` — on every host, like the reading width, since
// the browser skin hosts the same rail.
pub use openspec_app::events::EVENT_COMMIT_HISTORY_ENABLED_CHANGED;

// The review skip patterns' event, emitted directly by
// `set_review_skip_patterns` once a list is stored — on every host, like the
// Commit history switch, since the browser skin renders the same pull-request
// views and the same Settings.
pub use openspec_app::events::EVENT_REVIEW_SKIP_PATTERNS_CHANGED;

// Likewise the pull-request panels' position event, emitted directly by
// `set_bitbucket_panel_position` and `set_github_panel_position` with the
// provider in its payload. Every host renders the panels, so it is not
// macOS-only either. (The snapshots' own `bitbucket-pull-requests-updated` and
// `github-pull-requests-updated` need no re-export: they are `CacheEvent`s,
// forwarded through `event_envelope` below.)
pub use openspec_app::events::EVENT_PULL_REQUEST_PANEL_MOVED;

// Pane-toggle events and the Settings… request, emitted directly by the macOS
// application menu (`menu.rs` / `lib.rs`) rather than via the `CacheEvent`
// forwarder — same situation as the presentation-updated event above.
#[cfg(target_os = "macos")]
pub use openspec_app::events::{
    EVENT_OPEN_SETTINGS, EVENT_TOGGLE_COMMIT_RAIL, EVENT_TOGGLE_SIDEBAR,
};

/// The desktop-only event name for review-action invalidations
/// (`pull-request-actions`: *Review Action State Synchronises Only Native
/// Windows*, design D12). It deliberately does not live in
/// `openspec_app::events` with the shared names: the web server neither
/// subscribes to the action broadcast nor knows this name, so no SSE frame can
/// ever carry one.
pub const EVENT_REVIEW_ACTION_CHANGED: &str = "review-action-changed";

/// Whether a window with this label shows review-action state: the main window
/// and every detached `pull-request-*` window do; readers, image windows and
/// anything else do not. The browser skin and terminal receive no action event
/// at all, because the action broadcast is not a `ServiceNotice` and the web
/// server never subscribes to it.
pub fn receives_action_events(label: &str) -> bool {
    label == "main" || crate::pull_request_window::is_pull_request_label(label)
}

/// Subscribe to the watcher's `CacheEvent` stream and forward each variant to
/// the appropriate named Tauri event, using the shared `event_envelope` mapping
/// so the wire shape matches the web server's SSE bridge exactly. Spawns a
/// tokio task that lives as long as the broadcast channel is open.
pub fn spawn_event_forwarder(app: AppHandle, watcher: &WatcherManager) {
    let mut rx = watcher.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let (name, payload) = event_envelope(&event);
                    let _ = app.emit(name, payload);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

/// Subscribe to the document-watch stream and forward each change to the
/// `document-changed` Tauri event, through the shared `document_envelope` so
/// the wire shape matches the web server's SSE frame exactly.
///
/// A separate task from [`spawn_event_forwarder`] because it drains a separate
/// channel: a document change is not a cache change, and giving it its own
/// stream is what kept every existing `CacheEvent` consumer untouched.
pub fn spawn_document_forwarder(app: AppHandle, documents: &DocumentWatcher) {
    let mut rx = documents.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(change) => {
                    let (name, payload) = document_envelope(&change);
                    let _ = app.emit(name, payload);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

/// Subscribe to the service's notice broadcast and forward each notice —
/// `review-progress-changed` and `pull-request-provider-changed` — to every
/// window, through the shared `notice_envelope` so the wire shape matches the
/// web server's SSE frame exactly (`pull-request-viewer`: *Review Progress*,
/// *Provider Enabled Flags Stay Current*).
///
/// A third task, for the reason [`spawn_document_forwarder`] is a second: a
/// notice is neither a cache change nor a document change. The service raises
/// it whichever transport caused it, so a mark set or a provider switched off
/// from a tab of the embedded web server reaches every window here, as one
/// set here reaches every tab. A lagging receiver skips notices rather than
/// stopping: each only asks a view to re-read.
pub fn spawn_notice_forwarder(app: AppHandle, svc: &AppService) {
    let mut rx = svc.subscribe_notices();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(notice) => {
                    let (name, payload) = notice_envelope(&notice);
                    let _ = app.emit(name, payload);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

/// Subscribe to the service's separate action-notice broadcast and forward each
/// `ActionInvalidation` to the windows that show review state — the main
/// window and every detached pull-request window — under
/// [`EVENT_REVIEW_ACTION_CHANGED`] (`pull-request-actions`: *Review Action
/// State Synchronises Only Native Windows*, design D12).
///
/// A fourth task, and a deliberately separate channel: `ServiceNotice` is
/// bridged to browser SSE, so action state is not a notice variant. The web
/// server neither subscribes to `subscribe_action_notices` nor serialises
/// `ActionInvalidation`, which makes accidental exposure of draft existence,
/// prose or operation timing to served tabs structurally impossible. The
/// event carries only the canonical reference and a revision or phase; every
/// receiver re-reads action state instead of trusting the event as a payload.
///
/// A lagging receiver skips invalidations rather than stopping, exactly like
/// the notice forwarder: each one only asks a view to re-read, and a receiver
/// that missed one reads current state on mount anyway.
pub fn spawn_action_forwarder(app: AppHandle, svc: &AppService) {
    let mut rx = svc.subscribe_action_notices();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(invalidation) => {
                    for (label, window) in app.webview_windows() {
                        if receives_action_events(&label) {
                            let _ = window.emit(EVENT_REVIEW_ACTION_CHANGED, &invalidation);
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use openspec_app::events::{
        PullRequestProviderChangedPayload, ServiceNotice, EVENT_PULL_REQUEST_PROVIDER_CHANGED,
        EVENT_REVIEW_PROGRESS_CHANGED,
    };
    use openspec_app::review_actions::{ActionInvalidation, OperationPhase};
    use openspec_app::PullRequestReference;
    use serde_json::json;

    fn reference() -> PullRequestReference {
        PullRequestReference {
            provider: openspec_app::events::PullRequestProvider::Github,
            owner: "acme".to_string(),
            repo: "api".to_string(),
            number: 7,
        }
    }

    /// A detached window's label is `pull-request-<hash>`; the predicate must
    /// reach those and `main` alone, never a reader, an image window or any
    /// other label. This is the exact filter `spawn_action_forwarder` applies
    /// to the live windows.
    #[test]
    fn only_main_and_pull_request_windows_receive_action_events() {
        assert!(receives_action_events("main"));
        assert!(receives_action_events("pull-request-4f2a9c"));
        assert!(!receives_action_events("reader-4f2a9c"));
        assert!(!receives_action_events("image-4f2a9c"));
        assert!(!receives_action_events("settings"));
        assert!(!receives_action_events(""));
    }

    /// The invalidation is an invalidation, not a payload: a draft move names
    /// only the reference and the new revision, an operation move only the
    /// reference and the new phase. No summary, comment, reply, receipt,
    /// credential or provider body is present under any key.
    #[test]
    fn an_action_event_carries_no_prose_or_secret() {
        let draft = serde_json::to_value(ActionInvalidation::Draft {
            reference: reference(),
            revision: 12,
        })
        .unwrap();
        assert_eq!(
            draft,
            json!({
                "kind": "draft",
                "reference": {
                    "provider": "github",
                    "owner": "acme",
                    "repo": "api",
                    "number": 7
                },
                "revision": 12
            })
        );

        let operation = serde_json::to_value(ActionInvalidation::Operation {
            reference: reference(),
            phase: OperationPhase::Attempting,
        })
        .unwrap();
        assert_eq!(
            operation,
            json!({
                "kind": "operation",
                "reference": {
                    "provider": "github",
                    "owner": "acme",
                    "repo": "api",
                    "number": 7
                },
                "phase": "attempting"
            })
        );

        for payload in [draft, operation] {
            let text = payload.to_string();
            for forbidden in [
                "summary",
                "body",
                "items",
                "receipt",
                "credential",
                "token",
                "marker",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "action event leaked `{forbidden}`: {text}"
                );
            }
        }
    }

    /// The dedicated name is shell-only: it is neither of the shared
    /// service-notice names, so adding the action broadcast to the notice
    /// forwarder — or folding its event name into `notice_envelope` — would
    /// fail this pin. The frontend contract spells the same name in
    /// `src/types.ts`, ready for the listener task.
    #[test]
    fn the_action_event_is_separate_from_service_notices() {
        assert_ne!(EVENT_REVIEW_ACTION_CHANGED, EVENT_REVIEW_PROGRESS_CHANGED);
        assert_ne!(
            EVENT_REVIEW_ACTION_CHANGED,
            EVENT_PULL_REQUEST_PROVIDER_CHANGED
        );
        // The shared forwarder maps exactly `ServiceNotice` through
        // `notice_envelope`; neither of its current variants may ever carry the
        // action event name.
        let notices = [
            ServiceNotice::ReviewProgressChanged(reference()),
            ServiceNotice::PullRequestProviderChanged(PullRequestProviderChangedPayload {
                provider: openspec_app::events::PullRequestProvider::Github,
                enabled: true,
            }),
        ];
        for notice in &notices {
            assert_ne!(
                notice_envelope(notice).0,
                EVENT_REVIEW_ACTION_CHANGED,
                "the action event must not be a service notice"
            );
        }
        assert_eq!(EVENT_REVIEW_ACTION_CHANGED, "review-action-changed");
        let types = include_str!("../../../src/types.ts");
        assert!(
            types.contains("export const EVENT_REVIEW_ACTION_CHANGED = \"review-action-changed\""),
            "src/types.ts no longer pins the desktop action event name"
        );
    }
}
