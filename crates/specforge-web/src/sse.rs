//! `GET /api/events` — the one-way event stream.
//!
//! Bridges the watcher's `CacheEvent` broadcast (and the app-event channel for
//! `workspace-presentation-updated`) to Server-Sent Events. Each frame's `event:`
//! name and `data:` payload come from the shared `event_envelope`, so the wire
//! shape matches the desktop forwarder exactly and the frontend's existing
//! handlers fire unchanged. The browser's native `EventSource` reconnects on its
//! own, so this is a better fit than a bidirectional socket.

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::Stream;
use openspec_app::{document_envelope, event_envelope};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;

use crate::AppState;

/// `?client=<id>` — the page's own identifier, minted once per document load.
/// Optional so an older cached bundle still gets events; such a client simply
/// owns no document watches.
#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    client: Option<String>,
}

pub async fn events_handler(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    Sse::new(event_stream_for(&state, query.client)).keep_alive(KeepAlive::default())
}

/// How long a page may be without an event stream before its document watches
/// are released.
///
/// `EventSource` retries a dropped connection on its own — 3s by spec, backing
/// off from there — and the page is fully alive across that gap, still
/// displaying documents and still expecting them to stay fresh. Releasing the
/// instant a connection ends would silently blind it. This window is long
/// enough for a retry to land and short enough that a closed tab's watches do
/// not linger.
const RECONNECT_GRACE: Duration = Duration::from_secs(15);

/// Releases every document watch a page owns once that page has no event
/// stream left and has not come back.
///
/// The stream is dropped when the connection closes — including when the tab
/// is killed rather than closed politely, because that drops the TCP
/// connection too — so this is the one hook a frontend cannot skip by failing
/// to clean up after itself. What it must NOT do is treat a reconnect as a
/// departure, which is why it counts connections rather than acting on each
/// drop.
struct OwnerGuard {
    state: AppState,
    owner: String,
}

impl OwnerGuard {
    /// Register a new connection for `owner` and hand back the guard that
    /// retires it.
    fn open(state: &AppState, owner: String) -> Self {
        *state
            .document_clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(owner.clone())
            .or_insert(0) += 1;
        Self {
            state: state.clone(),
            owner,
        }
    }
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        let remaining = {
            let mut clients = self
                .state
                .document_clients
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let count = clients.entry(self.owner.clone()).or_insert(0);
            *count = count.saturating_sub(1);
            *count
        };
        if remaining > 0 {
            // A replacement connection is already open — this is a reconnect,
            // or two tabs sharing one id. Nothing to release.
            return;
        }
        let state = self.state.clone();
        let owner = std::mem::take(&mut self.owner);
        tokio::spawn(async move {
            release_after_grace(state, owner, RECONNECT_GRACE).await;
        });
    }
}

/// Release `owner`'s document watches once `grace` has passed, unless it has
/// reconnected in the meantime.
///
/// Split out and taking `grace` as an argument so the decision can be tested
/// without waiting on a real clock.
pub(crate) async fn release_after_grace(state: AppState, owner: String, grace: Duration) {
    tokio::time::sleep(grace).await;
    let departed = {
        let mut clients = state
            .document_clients
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match clients.get(&owner) {
            Some(0) | None => {
                clients.remove(&owner);
                true
            }
            // Came back inside the grace window: its registrations are still
            // exactly the ones it holds, so leave them alone.
            Some(_) => false,
        }
    };
    if departed {
        state.svc.release_document_owner(&owner);
    }
}

/// The merged event stream: the watcher's `CacheEvent` broadcast plus the
/// app-event channel. Both receivers subscribe eagerly (before the stream is
/// returned), so an event emitted right after this call is captured. Factored
/// out so it is testable without driving the full `Sse` response.
#[cfg(test)]
pub(crate) fn event_stream(state: &AppState) -> impl Stream<Item = Result<Event, Infallible>> {
    event_stream_for(state, None)
}

/// The merged stream, owning `client`'s document watches for as long as it
/// lives.
pub(crate) fn event_stream_for(
    state: &AppState,
    client: Option<String>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let mut cache_rx = state.svc.subscribe();
    let mut extra_rx = state.extra_tx.subscribe();
    let mut doc_rx = state.svc.documents.subscribe();
    let guard = client.map(|owner| OwnerGuard::open(state, owner));

    async_stream::stream! {
        // Moved into the stream so its Drop runs when the connection ends.
        let _guard = guard;
        loop {
            tokio::select! {
                event = cache_rx.recv() => match event {
                    Ok(event) => {
                        let (name, payload) = event_envelope(&event);
                        yield Ok(sse_event(name, &payload));
                    }
                    // A slow client may lag the broadcast; skip dropped events
                    // rather than tearing the stream down (the frontend re-reads
                    // current state on the next event it does receive).
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                },
                extra = extra_rx.recv() => match extra {
                    Ok((name, payload)) => yield Ok(sse_event(&name, &payload)),
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                },
                change = doc_rx.recv() => match change {
                    Ok(change) => {
                        let (name, payload) = document_envelope(&change);
                        yield Ok(sse_event(name, &payload));
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    // `continue` here would spin: a closed broadcast receiver
                    // returns immediately and forever, so `select!` would busy
                    // -loop. The document sender lives as long as the service,
                    // so a close means shutdown anyway.
                    Err(RecvError::Closed) => break,
                },
            }
        }
    }
}

/// Build a named SSE frame. The payloads are plain JSON, so `json_data` only
/// fails on the impossible serialize error; fall back to a `null` body so the
/// named event still reaches handlers that re-read via a command.
fn sse_event(name: &str, payload: &Value) -> Event {
    Event::default()
        .event(name)
        .json_data(payload)
        .unwrap_or_else(|_| Event::default().event(name).data("null"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;
    use futures::StreamExt;
    use openspec_core::CacheEvent;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::broadcast;

    fn test_state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let svc = openspec_app::AppService::bootstrap(dir.path().to_path_buf());
        let (extra_tx, _) = broadcast::channel(16);
        (
            AppState {
                svc,
                extra_tx,
                document_clients: Arc::new(Mutex::new(HashMap::new())),
            },
            dir,
        )
    }

    /// A registered workspace with one markdown file, so document watches can
    /// actually be taken against it.
    fn workspace_with_a_document(state: &AppState) -> (tempfile::TempDir, PathBuf) {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("openspec").join("changes")).unwrap();
        std::fs::write(root.join("a.md"), "# a").unwrap();
        state
            .svc
            .registry
            .lock()
            .unwrap()
            .register(root.clone())
            .expect("workspace registers");
        (ws, root)
    }

    /// The reconnect this whole mechanism exists for. `EventSource` retries a
    /// dropped connection on its own, and the page is alive across the gap: if
    /// the old connection's teardown released its watches, the page would keep
    /// rendering and never receive another document update.
    #[tokio::test]
    async fn a_second_connection_keeps_a_reconnecting_page_s_watches() {
        let (state, _dir) = test_state();
        let (_ws, root) = workspace_with_a_document(&state);
        state
            .svc
            .watch_document("page-1", root.clone(), "a.md".to_string())
            .await
            .unwrap();
        assert_eq!(state.svc.documents.registration_count(), 1);

        let first = Box::pin(event_stream_for(&state, Some("page-1".to_string())));
        let second = Box::pin(event_stream_for(&state, Some("page-1".to_string())));

        // The replacement is open before the old connection finishes closing,
        // which is exactly the ordering a browser reconnect produces.
        drop(first);
        release_after_grace(state.clone(), "page-1".to_string(), Duration::ZERO).await;

        assert_eq!(
            state.svc.documents.registration_count(),
            1,
            "a page that still holds a connection must keep its watches"
        );
        drop(second);
    }

    /// The other half: a page that really is gone loses its watches.
    #[tokio::test]
    async fn a_departed_page_loses_its_watches_after_the_grace() {
        let (state, _dir) = test_state();
        let (_ws, root) = workspace_with_a_document(&state);
        state
            .svc
            .watch_document("page-1", root.clone(), "a.md".to_string())
            .await
            .unwrap();

        let only = Box::pin(event_stream_for(&state, Some("page-1".to_string())));
        drop(only);
        release_after_grace(state.clone(), "page-1".to_string(), Duration::ZERO).await;

        assert_eq!(
            state.svc.documents.registration_count(),
            0,
            "a page with no connection left releases everything it held"
        );
        assert_eq!(state.svc.documents.watched_dir_count(), 0);
    }

    #[tokio::test]
    async fn emitted_cache_event_appears_on_stream() {
        let (state, _dir) = test_state();
        let mut stream = Box::pin(event_stream(&state));
        // Emit after the stream subscribed (event_stream subscribes eagerly).
        state.svc.watcher.emit(CacheEvent::GraphChanged {
            repo_id: PathBuf::from("/r/.git"),
        });
        let item = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("stream should yield before timeout");
        assert!(matches!(item, Some(Ok(_))));
    }

    /// The reading-width event rides the same generic `(name, Value)` channel,
    /// so the web transport needed no new machinery — this asserts the STREAM
    /// carries it. It publishes by hand and therefore says nothing about the
    /// producer: that `dispatch`'s `set_document_width` arm actually emits is
    /// covered by `dispatch::tests::set_document_width_emits_the_change_event`,
    /// which is where a deleted emit is caught.
    ///
    /// Named via the constant so a rename cannot leave this test passing
    /// against a name no longer in use.
    #[tokio::test]
    async fn document_width_event_appears_on_stream() {
        let (state, _dir) = test_state();
        let mut stream = Box::pin(event_stream(&state));
        let _ = state.extra_tx.send((
            openspec_app::events::EVENT_DOCUMENT_WIDTH_CHANGED.to_string(),
            Value::String("full".into()),
        ));
        let item = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("stream should yield before timeout");
        assert!(matches!(item, Some(Ok(_))));
    }

    #[tokio::test]
    async fn presentation_event_appears_on_stream() {
        let (state, _dir) = test_state();
        let mut stream = Box::pin(event_stream(&state));
        let _ = state
            .extra_tx
            .send(("workspace-presentation-updated".into(), Value::Null));
        let item = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("stream should yield before timeout");
        assert!(matches!(item, Some(Ok(_))));
    }
}
