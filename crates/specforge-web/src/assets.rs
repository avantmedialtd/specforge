//! Static serving of the React bundle.
//!
//! The same `dist/` the desktop app bundles is embedded here. In release builds
//! it is compiled into the binary; in debug builds rust-embed reads it from disk,
//! so a `bun run build` is picked up without recompiling the server. Unknown
//! paths fall back to `index.html` for client-side routing (SPA).

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

    // SPA fallback: any unknown, non-asset path renders the app shell.
    match Assets::get("index.html") {
        Some(content) => (
            [(header::CONTENT_TYPE, "text/html")],
            content.data.into_owned(),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            "web UI assets not found — build the frontend with `bun run build`",
        )
            .into_response(),
    }
}
