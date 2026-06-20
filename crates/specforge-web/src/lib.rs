//! The optional local web server for SpecForge.
//!
//! A transport adapter, not new behaviour: it wraps an [`openspec_app::AppService`]
//! (the same brain the desktop shell and terminal frontend render) with an HTTP
//! command endpoint and a one-way SSE event stream, and serves the same React
//! bundle the desktop app uses.
//!
//! Two entry points share one core ([`serve`]):
//! - **embedded** — the running desktop app calls `serve(svc.clone(), addr)` on
//!   the `AppService` it already holds, so the browser mirrors live desktop
//!   state with a single watcher and no second writer;
//! - **standalone** — the `specforge-serve` binary bootstraps its own
//!   `AppService` from the shared config dir and serves only the web UI.
//!
//! The server binds the loopback interface only and validates request origin,
//! so an unrelated web page in the user's browser cannot drive workspace
//! registration or artifact reads against the local filesystem.

mod assets;
pub mod dispatch;
mod sse;

use axum::{
    extract::State,
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use openspec_app::AppService;
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::sync::broadcast;

/// Shared server state. Cheaply cloneable — `AppService` shares its handles via
/// `Arc`, and `extra_tx` is a broadcast sender.
#[derive(Clone)]
pub struct AppState {
    pub svc: AppService,
    /// App-level events that are *not* derived from a `CacheEvent` (today only
    /// `workspace-presentation-updated`, emitted by the dispatch layer after a
    /// presentation write). The SSE stream merges this with the watcher's
    /// `CacheEvent` stream so the browser sees both.
    pub extra_tx: broadcast::Sender<(String, Value)>,
}

/// Build the HTTP application for an `AppService`: the command endpoint, the SSE
/// event stream, the embedded static assets, and the loopback trust-boundary
/// middleware. Pure and socket-free, so tests can drive it via `oneshot`.
pub fn router(svc: AppService) -> Router {
    let (extra_tx, _) = broadcast::channel(64);
    let state = AppState { svc, extra_tx };
    Router::new()
        .route("/api/invoke", post(invoke_handler))
        .route("/api/events", get(sse::events_handler))
        .fallback(assets::static_handler)
        .layer(middleware::from_fn(loopback_guard))
        .with_state(state)
}

/// Bind `addr` (must be loopback) and serve until the process exits.
pub async fn serve(svc: AppService, addr: SocketAddr) -> std::io::Result<()> {
    let app = router(svc);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

/// The `{ command, args }` request body — mirrors Tauri's `invoke(command, args)`.
#[derive(Deserialize)]
struct InvokeRequest {
    command: String,
    /// Absent / `null` for no-arg commands.
    #[serde(default)]
    args: Value,
}

/// `POST /api/invoke` — dispatch one command and return its JSON result, or a
/// `{ "error": "..." }` envelope with a 4xx status the frontend turns into a
/// thrown error (matching how a rejected Tauri command surfaces).
async fn invoke_handler(State(state): State<AppState>, Json(req): Json<InvokeRequest>) -> Response {
    match dispatch::dispatch(&state.svc, &state.extra_tx, &req.command, req.args).await {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": message })),
        )
            .into_response(),
    }
}

/// Loopback trust boundary. localhost is reachable by every page the user
/// visits, so binding `127.0.0.1` is not enough on its own:
///
/// - the `Host` header must be a loopback name — blocks DNS-rebinding, where an
///   attacker's hostname resolves to `127.0.0.1` (then `Host` is *their* name);
/// - the `Origin` header, when present, must also be loopback — blocks a
///   cross-origin page's `fetch`/`EventSource` (the browser sets `Origin` on
///   those). Same-origin navigations omit `Origin`, which is allowed.
async fn loopback_guard(req: Request<axum::body::Body>, next: Next) -> Response {
    let headers = req.headers();

    let host_ok = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(is_loopback_authority)
        .unwrap_or(false);
    if !host_ok {
        return (StatusCode::FORBIDDEN, "forbidden: non-loopback host").into_response();
    }

    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !is_loopback_authority(origin) {
            return (StatusCode::FORBIDDEN, "forbidden: cross-origin request").into_response();
        }
    }

    next.run(req).await
}

/// Whether an authority/origin string names a loopback host, ignoring scheme and
/// port (`http://localhost:4317` → `localhost`, `127.0.0.1:4317` → `127.0.0.1`,
/// `[::1]:4317` → `::1`).
fn is_loopback_authority(value: &str) -> bool {
    matches!(host_part(value), "localhost" | "127.0.0.1" | "::1")
}

/// Extract the bare host from a scheme/authority/port string.
fn host_part(value: &str) -> &str {
    let value = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .unwrap_or(value);
    // Drop any path (Origin is host-only, but be defensive).
    let value = value.split('/').next().unwrap_or(value);
    // IPv6 literal: `[::1]` or `[::1]:port`.
    if let Some(rest) = value.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    // `host:port` → `host`. No colon → the whole string.
    value.rsplit_once(':').map(|(h, _)| h).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_authorities_recognised() {
        for v in [
            "localhost",
            "localhost:4317",
            "127.0.0.1:4317",
            "http://localhost:4317",
            "https://127.0.0.1",
            "[::1]:4317",
            "http://[::1]:4317",
        ] {
            assert!(is_loopback_authority(v), "{v} should be loopback");
        }
    }

    #[test]
    fn non_loopback_authorities_rejected() {
        for v in [
            "evil.com",
            "evil.com:4317",
            "http://evil.com",
            "169.254.1.1:4317",
            "example.localhost.evil.com",
        ] {
            assert!(!is_loopback_authority(v), "{v} should be rejected");
        }
    }
}
