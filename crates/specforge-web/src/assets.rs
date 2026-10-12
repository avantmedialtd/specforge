//! Static serving of the React bundle.
//!
//! The same `dist/` the desktop app bundles is embedded here. In release builds
//! it is compiled into the binary and served borrowed, with no per-request
//! copy; in debug builds rust-embed reads it from disk on every request, so a
//! `bun run build` is picked up without recompiling or restarting the server.
//! Unknown paths fall back to `index.html` for client-side routing (SPA),
//! except inside the bundle's own static-asset namespace — see
//! [`is_static_asset_path`]. Every response that serves `index.html`, `304`s
//! included, carries the shell's own headers — see [`shell_response`].
//!
//! Every hit carries a caching policy chosen from the path that was served,
//! never from the request path (`web-ui`: *HTTP Caching of the Served
//! Bundle*): content-hashed `assets/*` are immutable for a year; the shell and
//! the stable-path root files (icons, favicon, manifest) must be revalidated,
//! against a weak entity tag built from the embedded file's SHA-256, and a
//! matching `If-None-Match` is answered `304` with no body. Misses and the
//! build hint are `no-store`.

use std::borrow::Cow;

use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::{EmbeddedFile, RustEmbed};

/// The built frontend bundle at the repo root. rust-embed resolves a relative
/// folder from the crate root, so `../../dist` is `<repo>/dist`. The directory
/// must exist at compile time (run `bun run build`); it may be served empty,
/// which the handler degrades to a build hint.
#[derive(RustEmbed)]
#[folder = "../../dist"]
struct Assets;

/// Root-level files a consumer may request without the document ever naming
/// them, so a miss must not be answered with the shell.
const WELL_KNOWN_ASSETS: &[&str] = &["favicon.ico", "favicon.svg", "manifest.webmanifest"];

/// `Cache-Control` for content-hashed files under `assets/`: a given path
/// never changes content, so a browser may reuse it for a year without asking.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// `Cache-Control` for the shell and the stable-path root files: reusable only
/// after the server confirms the entity tag, so a cached shell is never
/// presented without a live server behind it (`web-app-install`:
/// *Installability Adds No Service Worker*).
const REVALIDATE: &str = "no-cache";
/// `Cache-Control` for misses and the build hint, so a bundle built after the
/// server started takes effect on the next load.
const NO_STORE: &str = "no-store";

/// Whether a path names one of the bundle's root-level icon rasters.
///
/// Matched as a family rather than by exact name for two reasons: iOS probes
/// several `apple-touch-icon*` variants at the root regardless of what the
/// document declares, and a manifest entry pointing at an icon that was never
/// generated should fail as a missing image rather than quietly return HTML.
/// Requiring a `.png` suffix and no slash keeps client-side routes out.
fn is_root_icon(path: &str) -> bool {
    !path.contains('/')
        && std::path::Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        && (path.starts_with("apple-touch-icon") || path.starts_with("icon-"))
}

/// Whether a path names the bundle's own static-asset namespace.
///
/// This is deliberately an explicit set rather than a test for a file
/// extension. Addresses that deep-link into the UI are built from workspace and
/// change identifiers, which may themselves contain dots — an extension
/// heuristic would turn a working deep link into a 404 the first time somebody
/// named a change `v1.2`.
fn is_static_asset_path(path: &str) -> bool {
    if let Some(rest) = path.strip_prefix("assets/") {
        // The bundle's generated asset directory, one level deep.
        return !rest.is_empty();
    }
    WELL_KNOWN_ASSETS.contains(&path) || is_root_icon(path)
}

/// The caching policy for a bundled file, decided from the path that was
/// *served*: the SPA fallback serves `index.html`, so it always gets the
/// shell's policy whatever address was requested.
fn cache_policy(served: &str) -> &'static str {
    if served.starts_with("assets/") {
        IMMUTABLE
    } else {
        // `index.html` and every other root file — icons, favicon, manifest —
        // keep stable paths across builds, so they must be revalidated.
        REVALIDATE
    }
}

/// Whether a response of this content type is one the compression layer may
/// encode — the single source of truth both for the layer's predicate in
/// `lib.rs` and for the `Vary: Accept-Encoding` this module sets itself, so a
/// `304` and the `200` it stands for always agree on `Vary`.
///
/// Excluded: raster images (already compressed; SVG is text and stays
/// eligible), WOFF/WOFF2 fonts (already compressed), the SSE stream (an
/// encoder would buffer events until it flushed) and gRPC.
pub(crate) fn is_compressible(content_type: &str) -> bool {
    let ct = content_type.trim_start().to_ascii_lowercase();
    if ct.starts_with("image/") {
        return ct.starts_with("image/svg+xml");
    }
    !(ct.starts_with("font/woff")
        || ct.starts_with("text/event-stream")
        || ct.starts_with("application/grpc"))
}

/// The response body for embedded bytes without a per-request copy: release
/// builds hand out `&'static` data, wrapped as-is; debug builds read the file
/// from disk, and that freshly allocated buffer is moved, not cloned.
fn body_of(data: Cow<'static, [u8]>) -> Body {
    match data {
        Cow::Borrowed(bytes) => Body::from(axum::body::Bytes::from_static(bytes)),
        Cow::Owned(vec) => Body::from(axum::body::Bytes::from(vec)),
    }
}

/// A weak entity tag, `W/"<hex sha256>"`, from an embedded file's SHA-256.
///
/// Weak because the compression layer changes the bytes on the wire per
/// encoding while leaving the tag alone; a strong tag would claim
/// byte-identity across representations that differ. Release builds compute
/// the hash at compile time; debug builds hash the bytes just read from disk,
/// so a rebuilt `dist/` changes the tag with no restart.
fn weak_etag(sha256: [u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut tag = String::with_capacity(4 + 64);
    tag.push_str("W/\"");
    for byte in sha256 {
        let _ = write!(tag, "{byte:02x}");
    }
    tag.push('"');
    tag
}

/// The opaque part of an entity tag, quotes included, with any `W/` prefix
/// dropped; `None` when the value is not a quoted tag.
fn opaque_tag(tag: &str) -> Option<&str> {
    let tag = tag.trim();
    let tag = tag.strip_prefix("W/").unwrap_or(tag);
    (tag.len() >= 2 && tag.starts_with('"') && tag.ends_with('"')).then_some(tag)
}

/// Whether a request's `If-None-Match` field lines match `etag` under the
/// weak comparison RFC 9110 §13.1.2 requires for it: `*` matches any current
/// representation; otherwise any listed tag whose opaque part equals
/// `etag`'s, with `W/` on either side ignored. A list may be split across
/// several field lines, so every line is read. An absent header, and any
/// non-UTF-8 line or malformed entry, never matches, so the full body is sent.
pub(crate) fn if_none_match_matches<'a>(
    lines: impl IntoIterator<Item = &'a HeaderValue>,
    etag: &str,
) -> bool {
    let Some(current) = opaque_tag(etag) else {
        return false;
    };
    lines
        .into_iter()
        .filter_map(|line| line.to_str().ok())
        .any(|line| {
            line.trim() == "*"
                || line
                    .split(',')
                    .filter_map(opaque_tag)
                    .any(|candidate| candidate == current)
        })
}

/// The application shell, `index.html`, as every response serving it sends it
/// — `/`, `/index.html` and the fallback for every deep address alike, as a
/// `200` with `body` or a bodiless `304` — with the headers it carries in
/// every configuration, whatever the bind and whether or not Tailscale Serve
/// support is on (`web-ui`: *Localhost Trust Boundary*):
///
/// - `Content-Security-Policy: frame-ancestors 'none'` and
///   `X-Frame-Options: DENY`, so no other page can host the served UI in a
///   frame. The policy is that one directive alone: it governs which pages
///   may frame the shell, never what the shell loads, so remote images in the
///   user's own artifacts still load.
/// - `X-DNS-Prefetch-Control: off`, so the browser resolves no host named by a
///   link in rendered content, such as a pull request's description and
///   comments, before the link is followed.
///
/// Both statuses are built here, so a `304` cannot lose a header the `200`
/// carries. Each also carries the shell's `no-cache` policy, its entity tag
/// and `Vary: Accept-Encoding` (`web-ui`: *HTTP Caching of the Served
/// Bundle*).
fn shell_response(status: StatusCode, body: Option<Body>, etag: &str) -> Response {
    let mut response = match body {
        Some(body) => ([(header::CONTENT_TYPE, "text/html")], body).into_response(),
        None => Response::new(Body::empty()),
    };
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("frame-ancestors 'none'"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::X_DNS_PREFETCH_CONTROL,
        HeaderValue::from_static("off"),
    );
    set_validators(headers, REVALIDATE, etag, true);
    response
}

/// `Cache-Control`, `ETag` and, for a compressible type, `Vary:
/// Accept-Encoding`. The compression layer adds `Vary` only to a response it
/// actually encodes, and a bodiless `304` never is — so the handler sets it
/// on both, and the layer, finding it present, does not add a second.
fn set_validators(headers: &mut HeaderMap, policy: &'static str, etag: &str, vary: bool) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    if let Ok(tag) = HeaderValue::from_str(etag) {
        headers.insert(header::ETAG, tag);
    }
    if vary {
        headers.insert(
            header::VARY,
            HeaderValue::from_static(header::ACCEPT_ENCODING.as_str()),
        );
    }
}

/// Serve the shell: `304` when the client's tag still matches, else the full
/// document.
fn serve_shell(shell: EmbeddedFile, request: &HeaderMap) -> Response {
    let etag = weak_etag(shell.metadata.sha256_hash());
    if if_none_match_matches(request.get_all(header::IF_NONE_MATCH), &etag) {
        return shell_response(StatusCode::NOT_MODIFIED, None, &etag);
    }
    shell_response(StatusCode::OK, Some(body_of(shell.data)), &etag)
}

/// Serve a bundled file other than the shell under the policy its served path
/// earns: `304` when the client's tag still matches, else the file.
fn serve_file(path: &str, content: EmbeddedFile, request: &HeaderMap) -> Response {
    let etag = weak_etag(content.metadata.sha256_hash());
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let vary = is_compressible(mime.as_ref());
    let policy = cache_policy(path);
    let mut response = if if_none_match_matches(request.get_all(header::IF_NONE_MATCH), &etag) {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        (
            [(header::CONTENT_TYPE, mime.as_ref())],
            body_of(content.data),
        )
            .into_response()
    };
    set_validators(response.headers_mut(), policy, &etag, vary);
    response
}

/// A `404` the browser must not store, so a later build or a file that
/// appears is seen on the next request.
fn not_found(message: &'static str) -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CACHE_CONTROL, NO_STORE)],
        message,
    )
        .into_response()
}

/// Serve an embedded asset by path, falling back to `index.html` for client-side
/// routes, and to a build hint when no bundle is present. `headers` are the
/// request's, read for `If-None-Match`.
pub async fn static_handler(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = Assets::get(path) {
        if path == "index.html" {
            return serve_shell(content, &headers);
        }
        return serve_file(path, content, &headers);
    }

    // With no bundle at all, every path reports the build hint. The namespace
    // boundary below only applies once there is a bundle for a file to be
    // missing from.
    let Some(shell) = Assets::get("index.html") else {
        return not_found("web UI assets not found — build the frontend with `bun run build`");
    };

    // A consumer asking for an image or a manifest must never be handed an HTML
    // document under that request.
    if is_static_asset_path(path) {
        return not_found("not found");
    }

    // SPA fallback: any unknown, non-asset path renders the app shell.
    serve_shell(shell, &headers)
}

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderValue, StatusCode};

    use super::{
        cache_policy, if_none_match_matches, is_compressible, is_static_asset_path, not_found,
        weak_etag, IMMUTABLE, REVALIDATE,
    };

    const TAG: &str = "W/\"abc123\"";

    fn matches(header: &str) -> bool {
        if_none_match_matches([&HeaderValue::from_str(header).unwrap()], TAG)
    }

    #[test]
    fn if_none_match_matches_the_same_tag_in_either_form() {
        assert!(matches("W/\"abc123\""));
        // Weak comparison ignores `W/` on either side.
        assert!(matches("\"abc123\""));
        assert!(if_none_match_matches(
            [&HeaderValue::from_static("W/\"abc123\"")],
            "\"abc123\""
        ));
    }

    #[test]
    fn if_none_match_rejects_a_different_tag() {
        assert!(!matches("W/\"abc124\""));
        assert!(!matches("\"ABC123\""), "tags compare byte for byte");
        assert!(!matches("W/\"abc\""));
    }

    #[test]
    fn if_none_match_scans_a_list() {
        assert!(matches("\"zzz\", W/\"abc123\""));
        assert!(matches("W/\"one\",W/\"abc123\" , \"two\""));
        assert!(!matches("\"one\", \"two\""));
    }

    #[test]
    fn if_none_match_reads_a_list_split_across_field_lines() {
        let first = HeaderValue::from_static("\"one\", \"two\"");
        let second = HeaderValue::from_static("W/\"abc123\"");
        assert!(if_none_match_matches([&first, &second], TAG));
        assert!(if_none_match_matches([&second, &first], TAG));
        // A non-UTF-8 line does not hide a match on another.
        let opaque = HeaderValue::from_bytes(b"\"x\"\xff").unwrap();
        assert!(if_none_match_matches([&opaque, &second], TAG));
        assert!(!if_none_match_matches([&first, &opaque], TAG));
    }

    #[test]
    fn not_found_is_never_stored() {
        let response = not_found("not found");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(response.headers().get(header::ETAG).is_none());
    }

    #[test]
    fn if_none_match_wildcard_matches_any_current_representation() {
        assert!(matches("*"));
        assert!(matches("  *  "));
    }

    #[test]
    fn if_none_match_never_matches_a_missing_or_malformed_header() {
        assert!(!if_none_match_matches([], TAG));
        assert!(!matches(""));
        assert!(!matches("abc123"), "an unquoted tag is malformed");
        assert!(!matches("W/abc123"));
        assert!(!matches("\""));
        assert!(!matches(","));
        let opaque = HeaderValue::from_bytes(b"\"abc123\"\xff").unwrap();
        assert!(!if_none_match_matches([&opaque], TAG));
    }

    #[test]
    fn weak_etag_is_the_quoted_hex_digest() {
        let mut hash = [0u8; 32];
        hash[0] = 0xab;
        hash[31] = 0x01;
        let tag = weak_etag(hash);
        assert!(tag.starts_with("W/\"ab00"), "{tag}");
        assert!(tag.ends_with("01\""), "{tag}");
        assert_eq!(tag.len(), 2 + 2 + 64);
        assert!(if_none_match_matches(
            [&HeaderValue::from_str(&tag).unwrap()],
            &tag
        ));
    }

    #[test]
    fn the_served_path_decides_the_policy() {
        assert_eq!(cache_policy("assets/index-abc123.js"), IMMUTABLE);
        assert_eq!(cache_policy("index.html"), REVALIDATE);
        assert_eq!(cache_policy("favicon.svg"), REVALIDATE);
        assert_eq!(cache_policy("icon-192.png"), REVALIDATE);
        assert_eq!(cache_policy("manifest.webmanifest"), REVALIDATE);
    }

    #[test]
    fn compressible_types_exclude_encoded_media_and_the_event_stream() {
        for ct in [
            "text/html",
            "text/css",
            "text/javascript",
            "application/javascript",
            "application/json",
            "application/manifest+json",
            "image/svg+xml",
            "font/ttf",
        ] {
            assert!(is_compressible(ct), "{ct} should be compressible");
        }
        for ct in [
            "image/png",
            "image/x-icon",
            "font/woff",
            "font/woff2",
            "text/event-stream",
            "application/grpc",
        ] {
            assert!(!is_compressible(ct), "{ct} should not be compressible");
        }
    }

    #[test]
    fn generated_asset_directory_is_in_the_namespace() {
        assert!(is_static_asset_path("assets/index-abc123.js"));
        assert!(is_static_asset_path("assets/fonts/Inter.woff2"));
        // The bare directory names no asset.
        assert!(!is_static_asset_path("assets/"));
    }

    #[test]
    fn well_known_root_files_are_in_the_namespace() {
        for path in [
            "favicon.ico",
            "favicon.svg",
            "manifest.webmanifest",
            "icon-192.png",
            "icon-512.png",
            "icon-512-maskable.png",
        ] {
            assert!(is_static_asset_path(path), "{path} should be an asset path");
        }
    }

    #[test]
    fn root_icon_rasters_are_in_the_namespace_as_a_family() {
        // Declared today.
        assert!(is_static_asset_path("icon-192.png"));
        assert!(is_static_asset_path("icon-512.png"));
        assert!(is_static_asset_path("icon-512-maskable.png"));
        assert!(is_static_asset_path("apple-touch-icon.png"));
        // Probed by iOS whether or not the document names them.
        assert!(is_static_asset_path("apple-touch-icon-precomposed.png"));
        assert!(is_static_asset_path("apple-touch-icon-180x180.png"));
        // A manifest entry that was never generated must fail as a missing
        // image, not quietly return the shell.
        assert!(is_static_asset_path("icon-256.png"));
    }

    #[test]
    fn icon_shaped_routes_without_a_raster_suffix_are_not_assets() {
        // A client-side route is never a `.png`, so the family test cannot
        // swallow one.
        assert!(!is_static_asset_path("icon-gallery"));
        assert!(!is_static_asset_path("apple-touch-icon-settings"));
    }

    #[test]
    fn deep_addresses_are_not_in_the_namespace() {
        assert!(!is_static_asset_path("w/my-repo/change/add-thing"));
        assert!(!is_static_asset_path("index.html"));
        // A dot in an identifier must not be mistaken for an asset extension.
        assert!(!is_static_asset_path("w/my-repo/change/v1.2"));
        assert!(!is_static_asset_path("w/my-repo/file/mockup.html"));
        // A deep address whose last segment merely looks like an icon probe.
        assert!(!is_static_asset_path("w/repo/apple-touch-icon.png"));
        // A pull request's address, slash-less as `static_handler` passes it.
        assert!(!is_static_asset_path("pr/github/acme/api/42"));
    }
}
