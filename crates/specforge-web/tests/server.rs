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

/// A GET against `path` with a loopback `Host`, matching a real browser
/// navigation or asset fetch (no `Origin` — the authority guard only checks
/// it when present).
fn get_request(path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header(header::HOST, "localhost:4317")
        .body(Body::empty())
        .unwrap()
}

/// `<repo root>/dist` — the same directory `assets::static_handler`'s
/// `RustEmbed` reads from at runtime in a debug build (see
/// `crates/specforge-web/src/assets.rs`), located the same way: relative to
/// this crate's own manifest directory.
fn dist_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist")
}

fn index_html_bytes() -> Vec<u8> {
    std::fs::read(dist_dir().join("index.html"))
        .expect("dist/index.html must exist — run `bun run build` first")
}

/// One bundled asset's path relative to `dist/` (e.g. `assets/index-abc123.js`).
/// Vite content-hashes every filename under `dist/assets/`, so there is no
/// stable name to hardcode — discover a real one instead.
fn any_asset_path() -> String {
    let assets_dir = dist_dir().join("assets");
    let entry = std::fs::read_dir(&assets_dir)
        .expect("dist/assets must exist — run `bun run build` first")
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_file())
        .expect("dist/assets must contain at least one file");
    format!("assets/{}", entry.file_name().to_string_lossy())
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

// ---- Static asset serving (view-routing's Deep-Link Durability pin) -------
//
// `assets::static_handler` itself is unchanged by `add-view-routing` — these
// tests pin the behaviour the `web-ui` capability's *Deep-Link Durability of
// the Served Bundle* requirement now specifies, so a future change can't
// quietly drop it as dead weight.

#[tokio::test]
async fn a_deep_address_that_matches_no_bundled_asset_is_served_the_shell() {
    let (app, _dir) = test_router();
    // A realistic view-routing deep link — the server never understands the
    // address grammar itself; it just doesn't match a bundled asset path.
    let res = app
        .oneshot(get_request("/r/some-repo/some-change/proposal"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html",
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        &body[..],
        &index_html_bytes()[..],
        "an unmatched deep path must render the exact app shell"
    );
}

#[tokio::test]
async fn reloading_at_a_deep_address_still_works() {
    // *Reloading a deep address works* — a second independent request at the
    // same deep path (simulating a reload) must behave identically.
    let (app, _dir) = test_router();
    for _ in 0..2 {
        let res = app
            .clone()
            .oneshot(get_request("/w/myproject/add-thing/design"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html",
        );
    }
}

#[tokio::test]
async fn a_bundled_asset_path_is_served_as_itself_not_shadowed_by_the_fallback() {
    let (app, _dir) = test_router();
    let asset_path = any_asset_path();
    let res = app
        .oneshot(get_request(&format!("/{asset_path}")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_ne!(
        content_type, "text/html",
        "a real asset must be served with its own content type, not the shell's"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_ne!(
        &body[..],
        &index_html_bytes()[..],
        "a real asset's body must not be the shell — the fallback must not have shadowed it"
    );
}

// ---- Static-asset namespace boundary (web-ui's Deep-Link Durability) ------
//
// The shell fallback is bounded by the bundle's own static-asset namespace: a
// miss inside it is a 404, not an HTML document served under an image request.
// These tests are the only safety net for that boundary — `.cargo/mutants.toml`
// scopes the mutation gate to `openspec-core` and `openspec-app`, so a diff
// touching only this crate reports green without running anything.

/// Every root-level icon path the built document actually declares.
fn declared_icon_paths() -> Vec<String> {
    let html = String::from_utf8(index_html_bytes()).unwrap();
    html.split("href=\"/")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter(|p| {
            p.starts_with("favicon")
                || p.starts_with("apple-touch-icon")
                || p.starts_with("icon-")
                || p.ends_with(".webmanifest")
        })
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn the_document_declares_an_icon_set_and_a_manifest() {
    let html = String::from_utf8(index_html_bytes()).unwrap();
    assert!(
        html.contains(r#"rel="icon" href="/favicon.svg""#),
        "the document must declare a scalable icon"
    );
    assert!(
        html.contains(r#"rel="icon" href="/favicon.ico""#),
        "the document must declare a raster icon fallback"
    );
    assert!(
        html.contains(r#"rel="apple-touch-icon""#),
        "the document must declare an Apple touch icon"
    );
    assert!(
        html.contains(r#"rel="manifest""#),
        "the document must link its web app manifest"
    );
}

#[tokio::test]
async fn every_declared_icon_resolves_to_a_bundled_asset() {
    let (app, _dir) = test_router();
    let declared = declared_icon_paths();
    assert!(
        declared.len() >= 4,
        "expected the document to declare an icon set, found {declared:?}"
    );
    for path in declared {
        let res = app
            .clone()
            .oneshot(get_request(&format!("/{path}")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "/{path} must be bundled");
        let content_type = res
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_ne!(
            content_type, "text/html",
            "/{path} must be served as itself, not the shell"
        );
    }
}

#[tokio::test]
async fn a_missing_asset_in_the_static_namespace_is_not_answered_with_the_shell() {
    let (app, _dir) = test_router();
    // iOS probes this variant at the root whether or not the document names it.
    // Before the namespace boundary existed this returned 200 text/html.
    let res = app
        .oneshot(get_request("/apple-touch-icon-precomposed.png"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "a miss inside the asset namespace must be a 404"
    );
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    assert!(
        !content_type.starts_with("text/html"),
        "a missing image must not be answered with an HTML content type, got {content_type:?}"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_ne!(
        &body[..],
        &index_html_bytes()[..],
        "a missing image must not be answered with the app shell"
    );
}

#[tokio::test]
async fn a_deep_address_containing_a_dot_is_still_served_the_shell() {
    let (app, _dir) = test_router();
    // The namespace is an explicit set, not an extension test: a change named
    // `v1.2` must not be mistaken for a static asset.
    for path in [
        "/w/myproject/v1.2/proposal",
        "/w/myproject/change/mockup.html",
    ] {
        let res = app.clone().oneshot(get_request(path)).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "{path} deep-links into the UI and must render the shell"
        );
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            &body[..],
            &index_html_bytes()[..],
            "{path} must render the exact app shell"
        );
    }
}

#[tokio::test]
async fn the_manifest_is_served_as_itself_and_names_no_origin() {
    let (app, _dir) = test_router();
    let res = app
        .oneshot(get_request("/manifest.webmanifest"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_ne!(
        content_type, "text/html",
        "the manifest must be served as itself, not the shell"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let manifest: serde_json::Value = serde_json::from_slice(&body).expect("manifest must be JSON");

    // *Web App Manifest Is Origin-Agnostic*: one build is served from a
    // loopback address on a configurable port and from a Tailscale name, so a
    // start URL naming any origin would be correct on at most one of them.
    let start_url = manifest["start_url"].as_str().expect("start_url");
    let scope = manifest["scope"].as_str().expect("scope");
    for (field, value) in [("start_url", start_url), ("scope", scope)] {
        assert!(
            !value.contains("://") && !value.starts_with("//"),
            "{field} must not name an origin, got {value:?}"
        );
    }
    assert_eq!(manifest["display"], "standalone");

    // *Icon Set Serves Masked Installers*: a maskable entry alongside full-bleed
    // ones, since a mask would crop the illustration's edge-to-edge frame.
    let icons = manifest["icons"].as_array().expect("icons");
    let purposes: Vec<&str> = icons
        .iter()
        .map(|i| i["purpose"].as_str().unwrap_or("any"))
        .collect();
    assert!(
        purposes.contains(&"maskable"),
        "the manifest must declare a maskable icon, found {purposes:?}"
    );
    assert!(
        purposes.iter().any(|p| *p != "maskable"),
        "the manifest must also declare full-bleed icons, found {purposes:?}"
    );
    for icon in icons {
        let src = icon["src"].as_str().expect("icon src");
        assert!(
            !src.contains("://"),
            "icon src must not name an origin, got {src:?}"
        );
    }
}
