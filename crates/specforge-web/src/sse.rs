//! `GET /api/events` — the one-way event stream.
//!
//! Bridges the watcher's `CacheEvent` broadcast (and the app-event channel for
//! `workspace-presentation-updated`) to Server-Sent Events. Each frame's `event:`
//! name and `data:` payload come from the shared `event_envelope`, so the wire
//! shape matches the desktop forwarder exactly and the frontend's existing
//! handlers fire unchanged. The browser's native `EventSource` reconnects on its
//! own, so this is a better fit than a bidirectional socket.

use std::convert::Infallible;

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

/// Releases every document watch a page owns once its event stream ends. The
/// stream is dropped when the connection closes — including when the tab is
/// killed rather than closed politely, because that drops the TCP connection
/// too — so this is the one hook that cannot be skipped by a frontend failing
/// to clean up after itself.
struct OwnerGuard {
    svc: openspec_app::AppService,
    owner: String,
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        self.svc.release_document_owner(&self.owner);
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
    let guard = client.map(|owner| OwnerGuard {
        svc: state.svc.clone(),
        owner,
    });

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
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::sync::broadcast;

    fn test_state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let svc = openspec_app::AppService::bootstrap(dir.path().to_path_buf());
        let (extra_tx, _) = broadcast::channel(16);
        (AppState { svc, extra_tx }, dir)
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
