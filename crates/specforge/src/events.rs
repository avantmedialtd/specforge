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
use tauri::{AppHandle, Emitter};
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
