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
//!   state with a single watcher and no second writer. Always loopback.
//! - **standalone** — the `specforge-serve` binary bootstraps its own
//!   `AppService` from the shared config dir and serves only the web UI.
//!   Binds loopback by default; an operator may explicitly request a
//!   non-loopback bind on the command line.
//!
//! While bound to loopback, the server validates request `Host`/`Origin`
//! against an allowlist, so an unrelated web page in the user's browser cannot
//! drive workspace registration or artifact reads against the local
//! filesystem. An explicit non-loopback bind (standalone binary only)
//! disables that allowlist entirely — see `design.md`'s Decision 3 in the
//! `add-network-bind-serve` change for exactly what that trades away.

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
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
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
    /// How many event-stream connections each page currently holds.
    ///
    /// A page's document watches are released when its stream ends — but
    /// "ends" includes `EventSource`'s own reconnect after any transient drop,
    /// and the page is still very much alive on the other side of it. Counting
    /// connections is what tells a reconnect apart from a closed tab: the
    /// release only fires once a client has no connection left, and even then
    /// only after a grace period long enough for the browser's retry to land.
    /// See `sse::release_after_grace`.
    pub document_clients: Arc<Mutex<HashMap<String, usize>>>,
}

/// Build the HTTP application for an `AppService`, with the trust boundary set
/// for a **loopback** bind (today's default allowlist semantics). Pure and
/// socket-free, so existing tests can drive it via `oneshot` without naming a
/// bind address. A real, possibly non-loopback, address goes through
/// [`router_with_bind`] instead — [`serve`] does this for you.
pub fn router(svc: AppService) -> Router {
    router_with_bind(svc, IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
}

/// Build the HTTP application for an `AppService`, with the trust boundary
/// resolved for `bind`: a loopback address keeps today's allowlist; a
/// non-loopback address switches the guard to [`TrustMode::AnyAuthority`]
/// (see `design.md` Decision 3). Pure and socket-free, so tests can drive it
/// via `oneshot`.
pub fn router_with_bind(svc: AppService, bind: IpAddr) -> Router {
    let (extra_tx, _) = broadcast::channel(64);
    // Resolve the trust-boundary inputs once (discovery may shell out) before
    // the service moves into `AppState`.
    let guard = build_guard_config(&svc, bind);
    let state = AppState {
        svc,
        extra_tx,
        document_clients: Arc::new(Mutex::new(HashMap::new())),
    };
    Router::new()
        .route("/api/invoke", post(invoke_handler))
        .route("/api/events", get(sse::events_handler))
        .fallback(assets::static_handler)
        .layer(middleware::from_fn_with_state(guard, authority_guard))
        .with_state(state)
}

/// Gate 2 (`Host`/`Origin`) trust mode. `Allowlist` is today's behaviour: only
/// the listed authorities pass. `AnyAuthority` is the explicit-network-bind
/// bypass (see `design.md` Decision 3) — a two-variant enum rather than a
/// `"*"` sentinel pushed into the allowlist `Vec`, so the bypass is a branch a
/// reader has to see, not a magic string they have to already know about.
#[derive(Clone)]
enum TrustMode {
    Allowlist(Arc<Vec<String>>),
    AnyAuthority,
}

/// Per-request trust-boundary inputs, resolved once when the router is built.
#[derive(Clone)]
struct GuardConfig {
    /// Gate 2: which `Host`/`Origin` values are accepted.
    trust: TrustMode,
    /// Gate 3: Tailscale logins permitted for non-loopback (tailnet) requests.
    /// Empty = trust the whole tailnet. Always empty when `trust` is
    /// `AnyAuthority` — startup refuses that combination before the router is
    /// ever built (see [`login_gate_would_be_voided`] and `design.md`
    /// Decision 5).
    allowed_logins: Arc<Vec<String>>,
}

/// Build the guard inputs from settings and the resolved bind address: a
/// non-loopback bind yields [`TrustMode::AnyAuthority`] (gate 2 disabled —
/// `design.md` Decision 3); a loopback bind yields today's allowlist,
/// unchanged, plus the resolved tailnet name when Tailscale Serve access is
/// on. Tailscale discovery (which may shell `tailscale status`) happens here,
/// once, not per request — and is skipped entirely under `AnyAuthority`,
/// since the allowlist it would feed is bypassed anyway.
fn build_guard_config(svc: &AppService, bind: IpAddr) -> GuardConfig {
    let ts = svc.settings.web_config().tailscale;
    let allowed_logins = Arc::new(ts.allowed_logins);

    if !is_loopback_bind(bind) {
        return GuardConfig {
            trust: TrustMode::AnyAuthority,
            allowed_logins,
        };
    }

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
        trust: TrustMode::Allowlist(Arc::new(allowed)),
        allowed_logins,
    }
}

/// Whether `bind` is a loopback address — gate 1, who can open a socket at
/// all. Not to be confused with [`is_loopback_authority`], which classifies a
/// `Host`/`Origin` *string* (gate 2). `IpAddr::is_loopback` already implements
/// exactly this classification (`127.0.0.0/8` and `::1`; `0.0.0.0`/`::` are
/// "unspecified", not loopback) — named and exported so `specforge-serve`'s
/// startup checks and this crate's guard share one definition.
pub fn is_loopback_bind(bind: IpAddr) -> bool {
    bind.is_loopback()
}

/// Whether starting would silently defeat the configured Tailscale login
/// gate: a non-loopback bind combined with a non-empty login allow-list. That
/// gate's trust in the `Tailscale-User-Login` header depends entirely on the
/// server binding loopback (`design.md` Decision 5, and the `Tailscale Serve
/// Access` requirement) — once any peer can reach the port directly, the
/// header is forgeable, so the honest response is to refuse to start rather
/// than silently serve with the restriction no longer enforced. Pure and
/// unit-testable without a socket or an `AppService`.
pub fn login_gate_would_be_voided(bind: IpAddr, allowed_logins: &[String]) -> bool {
    !is_loopback_bind(bind) && !allowed_logins.is_empty()
}

/// Bind `addr` and serve until the process exits. `addr`'s interface decides
/// the trust boundary: loopback keeps the request-authority allowlist in
/// force; a non-loopback bind (standalone binary only — see
/// [`login_gate_would_be_voided`] for the startup check that must run first)
/// disables it, per `design.md` Decision 3.
pub async fn serve(svc: AppService, addr: SocketAddr) -> std::io::Result<()> {
    let app = router_with_bind(svc, addr.ip());
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

/// Configurable trust boundary. Reachability (the bind address, gate 1) and
/// origin defense (gates 2/3) are separate jobs; this enforces the latter:
///
/// - under [`TrustMode::Allowlist`] (loopback bind): the `Host` must be an
///   allowed authority — always a loopback name, plus the host's own tailnet
///   name when Tailscale Serve access is on. Blocks DNS-rebinding;
///   `tailscale serve` preserves the original `Host`, so the tailnet name
///   must be allowed for serve-proxied requests to pass. The `Origin`, when
///   present, must also be allowed — blocks a cross-origin page's
///   `fetch`/`EventSource`. The allowlist is specific names only, never a
///   wildcard, so relaxing reachability never relaxes this check. When a
///   login allow-list is configured, a non-loopback (tailnet) request must
///   additionally carry an allowed `Tailscale-User-Login`. That header is
///   trustworthy only because the server binds loopback — `tailscale serve`
///   is then the sole non-local path and it strips any client-supplied copy.
/// - under [`TrustMode::AnyAuthority`] (explicit non-loopback bind): `Host`
///   and `Origin` are accepted unconditionally — the operator asking for a
///   network bind has declared that network trusted (`design.md` Decision
///   3). The login gate (gate 3) is unreachable in this mode by
///   construction: startup already refused to combine a non-loopback bind
///   with a non-empty login allow-list (`login_gate_would_be_voided`), so
///   `allowed_logins` is guaranteed empty here.
async fn authority_guard(
    State(guard): State<GuardConfig>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let headers = req.headers();
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());

    let allowed = match &guard.trust {
        TrustMode::AnyAuthority => {
            // Gate 2 is deliberately bypassed here (`design.md` Decision 3).
            // Gate 3 must be unreachable too: startup refuses to start if a
            // non-loopback bind carries a non-empty login allow-list, so this
            // can only be empty. If it ever isn't, that invariant broke
            // upstream of this request — fail loudly in debug/test builds
            // rather than silently trusting a now-forgeable header.
            debug_assert!(
                guard.allowed_logins.is_empty(),
                "AnyAuthority must never carry a non-empty login allow-list; \
                 startup should have refused to start"
            );
            return next.run(req).await;
        }
        TrustMode::Allowlist(allowed) => allowed,
    };

    let host_ok = host
        .map(|h| is_allowed_authority(h, allowed))
        .unwrap_or(false);
    if !host_ok {
        return (StatusCode::FORBIDDEN, "forbidden: host not allowed").into_response();
    }

    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !is_allowed_authority(origin, allowed) {
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

    // ---- is_loopback_bind (gate 1: who can open a socket) -----------------

    #[test]
    fn loopback_binds_recognised() {
        for v in ["127.0.0.1", "127.0.0.42", "::1"] {
            let addr: IpAddr = v.parse().unwrap();
            assert!(is_loopback_bind(addr), "{v} should be loopback");
        }
    }

    #[test]
    fn non_loopback_binds_recognised() {
        for v in ["0.0.0.0", "192.168.1.5", "::"] {
            let addr: IpAddr = v.parse().unwrap();
            assert!(!is_loopback_bind(addr), "{v} should not be loopback");
        }
    }

    // ---- login_gate_would_be_voided (design.md Decision 5) ----------------

    #[test]
    fn login_gate_voided_only_when_non_loopback_bind_meets_configured_logins() {
        let logins = vec!["alice@example.com".to_string()];
        let none: Vec<String> = Vec::new();
        let non_loopback: IpAddr = "0.0.0.0".parse().unwrap();
        let loopback: IpAddr = "127.0.0.1".parse().unwrap();

        assert!(
            login_gate_would_be_voided(non_loopback, &logins),
            "non-loopback bind + configured logins must refuse to start"
        );
        assert!(
            !login_gate_would_be_voided(non_loopback, &none),
            "non-loopback bind + empty allow-list must be allowed"
        );
        assert!(
            !login_gate_would_be_voided(loopback, &logins),
            "loopback bind + configured logins must be allowed"
        );
        assert!(!login_gate_would_be_voided(loopback, &none));
    }
}
