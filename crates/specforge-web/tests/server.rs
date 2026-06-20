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
        .oneshot(invoke_request(r#"{"command":"get_active_count","args":{}}"#))
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
        .oneshot(invoke_request(r#"{"command":"get_launch_on_login","args":{}}"#))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["error"].as_str().unwrap().contains("launch-on-login"));
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
        .body(Body::from(r#"{"command":"get_active_count","args":{}}"#.to_string()))
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
        .body(Body::from(r#"{"command":"get_active_count","args":{}}"#.to_string()))
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
        .body(Body::from(r#"{"command":"get_active_count","args":{}}"#.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
