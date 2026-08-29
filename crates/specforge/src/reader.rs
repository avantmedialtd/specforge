//! Reader windows: one document, no navigation.
//!
//! A reader window loads the same bundle the main window does, with
//! `?reader=1` telling the frontend to mount the chromeless document surface
//! instead of the full shell. The address it should show rides in the same
//! query string, because the shell has no URL routing to put it in the path:
//! the asset protocol serves real bundled files and has no `index.html`
//! fallback for an unknown path, so `tauri://localhost/r/repo/file/README.md`
//! would simply 404. `Url::join` preserves the query, and the frontend reads it
//! once at mount.
//!
//! # Identity
//!
//! A reader's window label is derived from the address it shows, so asking for
//! a document that already has a window focuses it rather than opening a
//! second. Tauri labels admit only `[a-zA-Z0-9-/:_]`, so the address itself
//! cannot be one — hence the hash.
//!
//! # Closing
//!
//! Reader windows install **no** `CloseRequested` handler. That is the whole
//! mechanism: the main window has one, which hides it so the tray and watcher
//! survive, and a window without one is destroyed by the same close request.
//! One menu item, one shortcut, two correct behaviours, no branch on label.

use openspec_app::AppService;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Every reader window's label starts with this. The window-state plugin
/// filters on it (readers share one remembered size rather than persisting one
/// entry per document), and the frontend never sees it.
pub const READER_LABEL_PREFIX: &str = "reader-";

/// How far each new reader is offset from the topmost visible one, in logical
/// pixels, so windows stack visibly instead of landing exactly on top of one
/// another.
const CASCADE_STEP: f64 = 24.0;

/// A reader window must stay big enough to read a line of prose in.
const MIN_WIDTH: f64 = 360.0;
const MIN_HEIGHT: f64 = 320.0;

/// FNV-1a (32-bit), rendered base-36.
///
/// Deliberately the same algorithm as `shortHash` in `src/routing/slug.ts`, so
/// a document's desktop window label and the browser host's `window.open`
/// target name are derived one documented way rather than two. Not a security
/// primitive. A collision is handled rather than assumed away: `open_reader`
/// confirms an existing window's `at` parameter before focusing it, and gives
/// the requested document its own window when they disagree.
pub fn short_hash(value: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    // Byte-wise over UTF-8, matching the JS version for the ASCII addresses
    // this is used on; both sides hash the same encoded address path.
    for byte in value.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    to_base36(hash)
}

fn to_base36(mut value: u32) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while value > 0 {
        out.push(DIGITS[(value % 36) as usize]);
        value /= 36;
    }
    out.reverse();
    String::from_utf8(out).expect("base36 digits are ASCII")
}

/// The window label for a reader showing `address_path`.
pub fn reader_label(address_path: &str) -> String {
    format!("{READER_LABEL_PREFIX}{}", short_hash(address_path))
}

/// Whether `label` names a reader window.
pub fn is_reader_label(label: &str) -> bool {
    label.starts_with(READER_LABEL_PREFIX)
}

/// Percent-encode a query-parameter value.
///
/// The address arrives already percent-encoded per path segment (`encodeAddress`
/// does that), so its `%` bytes must themselves be escaped or the value decodes
/// to something else on the way back out.
fn encode_query_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Open — or focus — the reader window for `address_path`.
///
/// `title` is supplied by the frontend, which is the layer that knows how a
/// document names itself; this module does no resolution of its own.
pub fn open_reader(app: &AppHandle, address_path: &str, title: &str) -> tauri::Result<()> {
    let label = reader_label(address_path);

    // Asking twice for one document focuses the window it already has. A
    // reader that was minimised is restored, so "open it again" always ends
    // with the document actually visible.
    //
    // The label is a 32-bit hash, so two addresses CAN collide. Focusing on a
    // label match alone would then hand the user a window showing a different
    // document, silently — the module's usual mitigation ("the window renders
    // from the URL") does not help here, because the window that already
    // exists keeps rendering what it was opened with. Confirming the address
    // first turns a silent wrong answer into a correct second window.
    if let Some(existing) = app.get_webview_window(&label) {
        if existing
            .url()
            .is_ok_and(|url| shows_address(&url, address_path))
        {
            let _ = existing.unminimize();
            existing.show()?;
            existing.set_focus()?;
            return Ok(());
        }
        return open_reader_labelled(app, &format!("{label}-2"), address_path, title);
    }

    open_reader_labelled(app, &label, address_path, title)
}

/// Whether the window at `url` is the reader for `address_path` — compared on
/// the `at` parameter the window was opened with, which is the address itself
/// rather than a hash of it.
fn shows_address(url: &tauri::Url, address_path: &str) -> bool {
    url.query_pairs()
        .any(|(key, value)| key == "at" && value == address_path)
}

fn open_reader_labelled(
    app: &AppHandle,
    label: &str,
    address_path: &str,
    title: &str,
) -> tauri::Result<()> {
    let geometry = app.state::<AppService>().settings.reader_window();
    let url = format!(
        "index.html?reader=1&at={}",
        encode_query_component(address_path)
    );

    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(geometry.width, geometry.height)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .resizable(true)
        // Deliberately NOT the main window's overlay titlebar: a native one
        // shows the document's title and earns the window a place in the
        // system's own window management (on macOS, the Window menu's window
        // list and the window-cycling shortcut).
        .on_navigation(|url| {
            url.scheme() == "tauri" || (cfg!(dev) && url.host_str() == Some("localhost"))
        });

    if let Some((x, y)) = cascade_origin(app) {
        builder = builder.position(x, y);
    }

    builder.build()?;
    Ok(())
}

/// Where a new reader should open: offset from the topmost visible reader, or
/// `None` when there is none and the platform should place it.
///
/// Positions are read as physical pixels and divided by the window's own scale
/// factor, because the builder takes logical ones — mixing the two places the
/// window at the wrong offset on a HiDPI display and at the right one on a
/// 1:1 display, which is the kind of bug that only shows up on someone else's
/// monitor.
fn cascade_origin(app: &AppHandle) -> Option<(f64, f64)> {
    // `webview_windows()` is a HashMap, whose iteration order is randomised.
    // Taking the first match would cascade off an ARBITRARY reader, so a new
    // window could land exactly on top of one already there — defeating the
    // offset precisely when several readers are open, and doing it
    // unpredictably from one launch to the next. Anchoring on the
    // furthest-along reader is deterministic and keeps the stack marching in
    // one direction.
    let anchor = app
        .webview_windows()
        .into_iter()
        .filter(|(label, window)| is_reader_label(label) && window.is_visible().unwrap_or(false))
        .filter_map(|(_, window)| {
            let position = window.outer_position().ok()?;
            let scale = window.scale_factor().unwrap_or(1.0);
            Some((f64::from(position.x) / scale, f64::from(position.y) / scale))
        })
        .max_by(|a, b| {
            (a.0 + a.1)
                .partial_cmp(&(b.0 + b.1))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    Some((anchor.0 + CASCADE_STEP, anchor.1 + CASCADE_STEP))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_stable_and_document_specific() {
        let a = reader_label("/r/specforge/file/README.md");
        let b = reader_label("/r/specforge/file/README.md");
        let c = reader_label("/r/specforge/file/docs/other.md");
        assert_eq!(a, b, "the same address always names the same window");
        assert_ne!(a, c, "different documents get different windows");
        assert!(is_reader_label(&a));
    }

    #[test]
    fn labels_use_only_characters_tauri_accepts() {
        for address in [
            "/r/specforge/file/openspec/specs/web-ui/spec.md",
            "/w/notes/README.md",
            "/r/a/b/proposal",
            "",
        ] {
            let label = reader_label(address);
            assert!(
                label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_')),
                "label {label:?} contains a character Tauri rejects"
            );
        }
    }

    /// Cross-checked against `shortHash` in `src/routing/slug.ts`: the two must
    /// stay the same algorithm, and these vectors are what says so. They were
    /// produced by running that function, not by reimplementing it here — a
    /// second implementation of the thing under test would agree with itself
    /// and prove nothing.
    #[test]
    fn short_hash_matches_the_frontend_algorithm() {
        assert_eq!(short_hash(""), "ztntfp");
        assert_eq!(short_hash("a"), "1r9wi7g");
        assert_eq!(short_hash("hello"), "m3bicr");
        assert_eq!(short_hash("/r/specforge/file/README.md"), "6t8xsy");
    }

    #[test]
    fn query_encoding_escapes_existing_percent_escapes() {
        // `encodeAddress` percent-encodes each path segment, so the value
        // reaching the query may already contain escapes; they must survive a
        // round trip rather than being decoded a level early.
        assert_eq!(
            encode_query_component("/r/a%20b/file/x.md"),
            "%2Fr%2Fa%2520b%2Ffile%2Fx.md"
        );
        assert_eq!(encode_query_component("plain-._~"), "plain-._~");
    }

    #[test]
    fn an_existing_window_is_only_reused_when_it_shows_the_same_address() {
        let matching = tauri::Url::parse(
            "tauri://localhost/index.html?reader=1&at=%2Fw%2Fnotes%2Ffile%2Fa.md",
        )
        .unwrap();
        assert!(shows_address(&matching, "/w/notes/file/a.md"));
        // The collision case: same label, different document. Focusing here
        // would hand the user the wrong file with no error.
        assert!(!shows_address(&matching, "/w/notes/file/b.md"));
    }

    #[test]
    fn a_window_with_no_address_is_never_reused() {
        let bare = tauri::Url::parse("tauri://localhost/index.html?reader=1").unwrap();
        assert!(!shows_address(&bare, "/w/notes/file/a.md"));
        let main = tauri::Url::parse("tauri://localhost/index.html").unwrap();
        assert!(!shows_address(&main, "/w/notes/file/a.md"));
    }

    #[test]
    fn main_window_is_not_a_reader() {
        assert!(!is_reader_label("main"));
    }
}
