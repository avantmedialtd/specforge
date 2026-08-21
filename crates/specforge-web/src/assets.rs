//! Static serving of the React bundle.
//!
//! The same `dist/` the desktop app bundles is embedded here. In release builds
//! it is compiled into the binary; in debug builds rust-embed reads it from disk,
//! so a `bun run build` is picked up without recompiling the server. Unknown
//! paths fall back to `index.html` for client-side routing (SPA), except inside
//! the bundle's own static-asset namespace — see [`is_static_asset_path`].

use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

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

/// Whether a path names one of the bundle's root-level icon rasters.
///
/// Matched as a family rather than by exact name for two reasons: iOS probes
/// several `apple-touch-icon*` variants at the root regardless of what the
/// document declares, and a manifest entry pointing at an icon that was never
/// generated should fail as a missing image rather than quietly return HTML.
/// Requiring a `.png` suffix and no slash keeps client-side routes out.
fn is_root_icon(path: &str) -> bool {
    !path.contains('/')
        && path.ends_with(".png")
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

/// Serve an embedded asset by path, falling back to `index.html` for client-side
/// routes, and to a build hint when no bundle is present.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            [(header::CONTENT_TYPE, mime.as_ref())],
            content.data.into_owned(),
        )
            .into_response();
    }

    // With no bundle at all, every path reports the build hint. The namespace
    // boundary below only applies once there is a bundle for a file to be
    // missing from.
    let Some(shell) = Assets::get("index.html") else {
        return (
            StatusCode::NOT_FOUND,
            "web UI assets not found — build the frontend with `bun run build`",
        )
            .into_response();
    };

    // A consumer asking for an image or a manifest must never be handed an HTML
    // document under that request.
    if is_static_asset_path(path) {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    // SPA fallback: any unknown, non-asset path renders the app shell.
    (
        [(header::CONTENT_TYPE, "text/html")],
        shell.data.into_owned(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::is_static_asset_path;

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
    }
}
