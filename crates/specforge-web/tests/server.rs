//! HTTP-level tests driving the router via `oneshot` (no socket bound).
//!
//! Cover the command transport (round-trip + unknown-command rejection) and the
//! loopback trust boundary (cross-origin and non-loopback host refused,
//! same-origin loopback allowed). The `CacheEvent → SSE` wiring is covered by
//! the in-crate `sse` tests; the name/payload mapping by `openspec-app`'s
//! `event_envelope` tests.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn test_router() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let svc = openspec_app::AppService::bootstrap(dir.path().to_path_buf());
    (specforge_web::router(svc), dir)
}

fn invoke_request(body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "localhost:4317")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn known_command_round_trips() {
    let (app, _dir) = test_router();
    // Empty registry → 0 active logical changes.
    let res = app
        .oneshot(invoke_request(
            r#"{"command":"get_active_count","args":{}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"0");
}

#[tokio::test]
async fn list_workspaces_round_trips_to_empty_array() {
    let (app, _dir) = test_router();
    let res = app
        .oneshot(invoke_request(r#"{"command":"list_workspaces","args":{}}"#))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"[]");
}

#[tokio::test]
async fn unknown_command_is_rejected() {
    let (app, _dir) = test_router();
    let res = app
        .oneshot(invoke_request(r#"{"command":"bogus","args":{}}"#))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["error"].as_str().unwrap().contains("unknown command"));
}

#[tokio::test]
async fn desktop_only_command_is_rejected_with_a_clear_message() {
    let (app, _dir) = test_router();
    let res = app
        .oneshot(invoke_request(
            r#"{"command":"get_launch_on_login","args":{}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["error"].as_str().unwrap().contains("launch-on-login"));
}

/// The artifact-link open operation is a deliberate carve-out from the
/// command-transport mirror contract (`web-ui` capability, *Link Handling in
/// the Browser Skin*): it acts on the *serving host's* filesystem/OS, so it
/// must never be reachable from a browser request. A regression guard against
/// some future change accidentally wiring a mirror arm for it.
#[tokio::test]
async fn open_artifact_link_is_not_mirrored() {
    let (app, _dir) = test_router();
    let res = app
        .oneshot(invoke_request(
            r#"{"command":"open_artifact_link","args":{"root":"/tmp","basePath":"a.md","href":"./x.html"}}"#,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("not available in the web UI"),
        "must be refused as unsupported, not silently ignored: {value}"
    );
}

#[tokio::test]
async fn cross_origin_is_forbidden() {
    let (app, _dir) = test_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "localhost:4317")
        .header(header::ORIGIN, "http://evil.com")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"command":"get_active_count","args":{}}"#.to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_loopback_host_is_forbidden() {
    let (app, _dir) = test_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "evil.com")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"command":"get_active_count","args":{}}"#.to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn same_origin_loopback_is_allowed() {
    let (app, _dir) = test_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "127.0.0.1:4317")
        .header(header::ORIGIN, "http://127.0.0.1:4317")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"command":"get_active_count","args":{}}"#.to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ---- Tailscale Serve access ------------------------------------------------

const TS_NAME: &str = "box.tailnet.ts.net";

/// A router with Tailscale Serve access enabled and a *manual* tailnet name
/// (so the test never shells out to `tailscale`).
fn tailscale_router(logins: &[&str]) -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let svc = openspec_app::AppService::bootstrap(dir.path().to_path_buf());
    svc.settings.set_web_tailscale_enabled(true).unwrap();
    svc.settings
        .set_web_tailscale_name(Some(TS_NAME.to_string()))
        .unwrap();
    if !logins.is_empty() {
        svc.settings
            .set_web_tailscale_allowed_logins(logins.iter().map(|s| s.to_string()).collect())
            .unwrap();
    }
    (specforge_web::router(svc), dir)
}

/// Build an invoke request with an explicit Host and optional Origin / login.
fn req(host: &str, origin: Option<&str>, login: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, host)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(o) = origin {
        b = b.header(header::ORIGIN, o);
    }
    if let Some(l) = login {
        b = b.header("tailscale-user-login", l);
    }
    b.body(Body::from(
        r#"{"command":"get_active_count","args":{}}"#.to_string(),
    ))
    .unwrap()
}

#[tokio::test]
async fn tailnet_host_is_refused_when_tailscale_disabled() {
    let (app, _dir) = test_router();
    let res = app
        .oneshot(req(TS_NAME, Some(&format!("https://{TS_NAME}")), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn tailnet_request_is_allowed_when_enabled() {
    let (app, _dir) = tailscale_router(&[]);
    let res = app
        .oneshot(req(TS_NAME, Some(&format!("https://{TS_NAME}")), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn loopback_still_allowed_with_tailscale_enabled() {
    let (app, _dir) = tailscale_router(&[]);
    let res = app
        .oneshot(req("127.0.0.1:4317", Some("http://127.0.0.1:4317"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn cross_origin_still_refused_with_tailscale_enabled() {
    let (app, _dir) = tailscale_router(&[]);
    let res = app
        .oneshot(req(TS_NAME, Some("https://evil.com"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn other_tailnet_name_is_refused() {
    let (app, _dir) = tailscale_router(&[]);
    let res = app
        .oneshot(req("other.tailnet.ts.net", None, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn allowed_login_passes_the_identity_gate() {
    let (app, _dir) = tailscale_router(&["alice@example.com"]);
    let res = app
        .oneshot(req(
            TS_NAME,
            Some(&format!("https://{TS_NAME}")),
            Some("alice@example.com"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn non_listed_login_is_refused() {
    let (app, _dir) = tailscale_router(&["alice@example.com"]);
    let res = app
        .oneshot(req(
            TS_NAME,
            Some(&format!("https://{TS_NAME}")),
            Some("bob@example.com"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn missing_login_is_refused_when_allowlist_configured() {
    let (app, _dir) = tailscale_router(&["alice@example.com"]);
    let res = app
        .oneshot(req(TS_NAME, Some(&format!("https://{TS_NAME}")), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn loopback_never_requires_a_login() {
    // Even with a login allow-list set, loopback (desktop / SSH-tunnel) is exempt.
    let (app, _dir) = tailscale_router(&["alice@example.com"]);
    let res = app
        .oneshot(req("localhost:4317", Some("http://localhost:4317"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ---- AnyAuthority (explicit non-loopback bind, design.md Decision 3) ------

/// A router built with an explicit non-loopback bind, so the guard runs under
/// `AnyAuthority` instead of the loopback allowlist.
fn any_authority_router() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let svc = openspec_app::AppService::bootstrap(dir.path().to_path_buf());
    (
        specforge_web::router_with_bind(svc, "0.0.0.0".parse().unwrap()),
        dir,
    )
}

#[tokio::test]
async fn any_authority_accepts_arbitrary_host_and_origin() {
    let (app, _dir) = any_authority_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "evil.example:4317")
        .header(header::ORIGIN, "http://evil.example:4317")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"command":"get_active_count","args":{}}"#.to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn default_allowlist_forbids_the_same_request_any_authority_would_allow() {
    // Same request shape as `any_authority_accepts_arbitrary_host_and_origin`,
    // through the default (loopback) router — existing behaviour must be
    // completely untouched by the `AnyAuthority` addition.
    let (app, _dir) = test_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "evil.example:4317")
        .header(header::ORIGIN, "http://evil.example:4317")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"command":"get_active_count","args":{}}"#.to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn any_authority_still_refuses_open_artifact_link() {
    // AnyAuthority only bypasses gates 2/3 (Host/Origin/login); the dispatch
    // table's own refusals for host-effectful commands are untouched.
    let (app, _dir) = any_authority_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "evil.example:4317")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"command":"open_artifact_link","args":{"root":"/tmp","basePath":"a.md","href":"./x.html"}}"#
                .to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("not available in the web UI"),
        "must still be refused as unsupported: {value}"
    );
}

#[tokio::test]
async fn any_authority_still_refuses_launch_on_login() {
    let (app, _dir) = any_authority_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "evil.example:4317")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"command":"get_launch_on_login","args":{}}"#.to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn any_authority_still_refuses_unknown_command() {
    let (app, _dir) = any_authority_router();
    let req = Request::builder()
        .method("POST")
        .uri("/api/invoke")
        .header(header::HOST, "evil.example:4317")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"command":"bogus","args":{}}"#.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
