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
pub mod tailscale;

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
use std::sync::Arc;
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
/// event stream, the embedded static assets, and the configurable trust-boundary
/// middleware. Pure and socket-free, so tests can drive it via `oneshot`.
pub fn router(svc: AppService) -> Router {
    let (extra_tx, _) = broadcast::channel(64);
    // Resolve the trust-boundary inputs once (discovery shells out) before the
    // service moves into `AppState`.
    let guard = build_guard_config(&svc);
    let state = AppState { svc, extra_tx };
    Router::new()
        .route("/api/invoke", post(invoke_handler))
        .route("/api/events", get(sse::events_handler))
        .fallback(assets::static_handler)
        .layer(middleware::from_fn_with_state(guard, authority_guard))
        .with_state(state)
}

/// Per-request trust-boundary inputs, resolved once when the router is built.
#[derive(Clone)]
struct GuardConfig {
    /// Authorities accepted on `Host`/`Origin`: always the loopback names, plus
    /// the host's own tailnet name when Tailscale Serve access is enabled and
    /// resolvable.
    allowed: Arc<Vec<String>>,
    /// Tailscale logins permitted for non-loopback (tailnet) requests. Empty =
    /// trust the whole tailnet.
    allowed_logins: Arc<Vec<String>>,
}

/// Build the guard inputs from settings: the loopback authorities always, plus
/// the resolved tailnet name when Tailscale Serve access is on. Resolution
/// (which may shell `tailscale status`) happens here, once, not per request.
fn build_guard_config(svc: &AppService) -> GuardConfig {
    let ts = svc.settings.web_config().tailscale;
    let mut allowed = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    if ts.enabled {
        if let Some(name) = tailscale::resolve_name(ts.name.as_deref()) {
            allowed.push(name);
        }
    }
    GuardConfig {
        allowed: Arc::new(allowed),
        allowed_logins: Arc::new(ts.allowed_logins),
    }
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

/// Configurable trust boundary. Reachability (the loopback bind) and origin
/// defense are separate jobs; this enforces the origin defense:
///
/// - the `Host` must be an allowed authority — always a loopback name, plus the
///   host's own tailnet name when Tailscale Serve access is on. Blocks
///   DNS-rebinding; `tailscale serve` preserves the original `Host`, so the
///   tailnet name must be allowed for serve-proxied requests to pass.
/// - the `Origin`, when present, must also be allowed — blocks a cross-origin
///   page's `fetch`/`EventSource`. The allowlist is specific names only, never a
///   wildcard, so relaxing reachability never relaxes this check.
/// - when a login allow-list is configured, a non-loopback (tailnet) request
///   must additionally carry an allowed `Tailscale-User-Login`. That header is
///   trustworthy only because the server binds loopback — `tailscale serve` is
///   then the sole non-local path and it strips any client-supplied copy.
async fn authority_guard(
    State(guard): State<GuardConfig>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let headers = req.headers();

    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let host_ok = host
        .map(|h| is_allowed_authority(h, &guard.allowed))
        .unwrap_or(false);
    if !host_ok {
        return (StatusCode::FORBIDDEN, "forbidden: host not allowed").into_response();
    }

    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !is_allowed_authority(origin, &guard.allowed) {
            return (StatusCode::FORBIDDEN, "forbidden: cross-origin request").into_response();
        }
    }

    // Per-user gate: a non-loopback (tailnet) request must carry an allowed
    // Tailscale identity when a login allow-list is configured. Loopback (the
    // desktop / SSH-tunnel path) never requires a login.
    let host_is_loopback = host.map(is_loopback_authority).unwrap_or(false);
    if !host_is_loopback && !guard.allowed_logins.is_empty() {
        let authorized = headers
            .get("tailscale-user-login")
            .and_then(|v| v.to_str().ok())
            .map(|login| {
                guard
                    .allowed_logins
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(login))
            })
            .unwrap_or(false);
        if !authorized {
            return (StatusCode::FORBIDDEN, "forbidden: user not authorized").into_response();
        }
    }

    next.run(req).await
}

/// Whether an authority/origin string matches one of the allowed names, ignoring
/// scheme, port, and ASCII case.
fn is_allowed_authority(value: &str, allowed: &[String]) -> bool {
    let host = host_part(value);
    allowed.iter().any(|a| a.eq_ignore_ascii_case(host))
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
