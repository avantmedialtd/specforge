//! Tauri event surface for cache changes.
//!
//! The frontend listens for these named events. Their *names* and *payload
//! shapes* now live in `openspec_app::events` (above both frontends) so the web
//! server's SSE bridge reproduces them identically — this module re-exports them
//! (so existing call sites keep working) and owns only the Tauri-specific
//! forwarding sink, which maps each `CacheEvent` through the shared
//! `event_envelope` and emits it as a Tauri event.

use openspec_app::event_envelope;
use openspec_core::WatcherManager;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;

// The presentation-updated event is emitted directly by a command (not via the
// `CacheEvent` forwarder), so this crate still references it by name. The other
// event names now live in `openspec_app::events` and are consumed there by
// `event_envelope`; re-export only what this crate uses to avoid dead re-exports.
pub use openspec_app::events::EVENT_WORKSPACE_PRESENTATION_UPDATED;

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
