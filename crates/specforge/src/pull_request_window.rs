//! The pull-request window: one pull request shown apart from the main window
//! (`pull-request-viewer`: *Pull-Request Window*, *Pull-Request Window Title*,
//! *Pull-Request Window Geometry*, *Pull-Request Window Permissions*; design
//! D3).
//!
//! The detached-window helper's second kind (see `reader.rs`, whose machinery
//! it shares): a window labelled `pull-request-<hash>` that loads
//! `index.html?pullRequest=1&at=<address>`, focused rather than opened twice,
//! with a native titlebar and no `CloseRequested` handler, so closing it
//! destroys it. What it adds to a reader:
//!
//! - **Size.** Pull-request windows share one remembered size of their own
//!   (`AppSettings::pull_request_window`), apart from the readers', clamped to
//!   the work area of the launching window's monitor and never smaller than
//!   600×400. A new one is offset only from visible pull-request windows.
//! - **Title.** A stranger wrote the pull request's title, so the window's
//!   title passes through `openspec_app::sanitize_window_title` when the
//!   window is built, and again whenever the page retitles itself, through the
//!   builder's title-change hook. Rust sets it, so the window needs no
//!   permission to set its own title.
//! - **Permissions.** `capabilities/pull-request.json` grants exactly what the
//!   window uses — listening, unlistening and closing itself — and none of the
//!   dialog, autostart, notification, menu or tray permissions `default.json`
//!   grants the main window and readers. Its content-security policy is the
//!   page's own, installed by `main.tsx` before the root renders: the shell
//!   sets none, so the main window and readers load what they always have.

use std::sync::Mutex;

use openspec_app::{
    sanitize_window_title, SettingsStore, PULL_REQUEST_WINDOW_MIN_HEIGHT,
    PULL_REQUEST_WINDOW_MIN_WIDTH,
};
use tauri::{AppHandle, Manager, WebviewWindow, Window};

use crate::reader::{open_detached, short_hash, DetachedKind};

/// Every pull-request window's label starts with this. Its own capability
/// matches it, `default.json` never does, and the window-state plugin skips
/// it, so no per-pull-request entry accumulates.
pub const PULL_REQUEST_LABEL_PREFIX: &str = "pull-request-";

/// The pull-request window, the detached-window helper's second kind.
pub const PULL_REQUEST: DetachedKind = DetachedKind {
    label: pull_request_label,
    owns: is_pull_request_label,
    query_flag: "pullRequest",
    min_size: (
        PULL_REQUEST_WINDOW_MIN_WIDTH,
        PULL_REQUEST_WINDOW_MIN_HEIGHT,
    ),
    size: pull_request_size,
    clamped_to_work_area: true,
};

/// The window label for the pull-request window showing `address_path`, an
/// encoded pull-request address. The browser skin names the pull request's
/// tab from the same hash.
pub fn pull_request_label(address_path: &str) -> String {
    format!("{PULL_REQUEST_LABEL_PREFIX}{}", short_hash(address_path))
}

/// Whether `label` names a pull-request window.
pub fn is_pull_request_label(label: &str) -> bool {
    label.starts_with(PULL_REQUEST_LABEL_PREFIX)
}

fn pull_request_size(settings: &SettingsStore) -> (f64, f64) {
    let geometry = settings.pull_request_window();
    (geometry.width, geometry.height)
}

/// Held from looking for the open window until a new one is built. The
/// command that opens a pull-request window is async, so two quick asks for
/// one pull request run side by side, and without it both could miss the
/// window the other is building. A reader needs none: its command runs on the
/// main thread, one at a time.
static OPENING: Mutex<()> = Mutex::new(());

/// Open — or focus — the pull-request window for `address_path`, asked for by
/// the window `launcher`, whose monitor's work area a new window fits.
///
/// `title` is the frontend's `pullRequestTitle`, which names the window from
/// the moment it is built. It is sanitised here all the same: the page is
/// never the only guard.
pub fn open_pull_request_window(
    app: &AppHandle,
    launcher: &Window,
    address_path: &str,
    title: &str,
) -> tauri::Result<()> {
    let _opening = OPENING.lock().unwrap_or_else(|e| e.into_inner());
    open_detached(
        app,
        &PULL_REQUEST,
        address_path,
        &sanitize_window_title(title),
        Some(launcher),
        |builder| builder.on_document_title_changed(follow_document_title),
    )
}

/// The builder's title-change hook: the native title follows the page's, as
/// [`followed_title`] decides, set from the Rust side.
fn follow_document_title(window: WebviewWindow, document_title: String) {
    let shell_title = &window.app_handle().package_info().name;
    if let Some(title) = followed_title(&document_title, shell_title) {
        let _ = window.set_title(&title);
    }
}

/// The title a pull-request window takes when its page's title becomes
/// `document_title`: that title, sanitised again. `None` keeps the title the
/// window has — for a title that sanitises to nothing, which would blank the
/// titlebar, and for `shell_title`, the product name every page carries as
/// `index.html`'s own title from the moment it parses until the root names its
/// window, which would otherwise flash in the titlebar of every pull-request
/// window as it opens.
fn followed_title(document_title: &str, shell_title: &str) -> Option<String> {
    let title = sanitize_window_title(document_title);
    (!title.is_empty() && title != shell_title).then_some(title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::{
        allows_navigation, cascade_from, clamp_to_work_area, is_detached_label, reader_label,
        READER,
    };
    use openspec_app::WINDOW_TITLE_CAP;
    use serde_json::Value;

    const ADDRESS: &str = "/pr/github/acme/api/42";

    #[test]
    fn the_label_and_url_name_the_address() {
        let label = pull_request_label(ADDRESS);
        assert_eq!(label, format!("pull-request-{}", short_hash(ADDRESS)));
        assert_eq!((PULL_REQUEST.label)(ADDRESS), label, "the kind's label");
        assert_ne!(
            label,
            pull_request_label("/pr/github/acme/api/43"),
            "different pull requests get different windows"
        );

        let url = PULL_REQUEST.url(ADDRESS);
        assert_eq!(
            url,
            "index.html?pullRequest=1&at=%2Fpr%2Fgithub%2Facme%2Fapi%2F42"
        );
        // What the page reads back: the flag `windowKind` selects the root by,
        // and the address `at` carries, not decoded a level early.
        let spelt = "/pr/bitbucket/my%20team/api/7";
        let page =
            tauri::Url::parse(&format!("tauri://localhost/{}", PULL_REQUEST.url(spelt))).unwrap();
        let query: Vec<(String, String)> = page.query_pairs().into_owned().collect();
        assert_eq!(
            query,
            [
                ("pullRequest".to_string(), "1".to_string()),
                ("at".to_string(), spelt.to_string()),
            ]
        );
    }

    #[test]
    fn labels_use_only_characters_tauri_accepts() {
        for address in [
            ADDRESS,
            "/pr/bitbucket/acme-team/api.service/7",
            "/pr/github/a%20b/c/1",
            "",
        ] {
            let label = pull_request_label(address);
            // A colliding pull request's second window too.
            for label in [label.clone(), format!("{label}-2")] {
                assert!(
                    label
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_')),
                    "label {label:?} contains a character Tauri rejects"
                );
                assert!(is_pull_request_label(&label));
            }
        }
    }

    #[test]
    fn a_small_work_area_clamps_the_remembered_size() {
        assert_eq!(
            clamp_to_work_area((1280.0, 860.0), (1024.0, 700.0)),
            (1024.0, 700.0)
        );
        assert_eq!(
            clamp_to_work_area((1280.0, 860.0), (2560.0, 1415.0)),
            (1280.0, 860.0),
            "a larger work area leaves the size alone"
        );
        assert_eq!(
            clamp_to_work_area((1280.0, 860.0), (1440.0, 700.0)),
            (1280.0, 700.0),
            "each dimension is clamped on its own"
        );
    }

    /// Each kind opens at its own remembered size, never the other's, and the
    /// pull-request setter floors what it stores at the window's own minimum,
    /// 600×400, so a stored size always fits it. Only the pull-request window
    /// is clamped to the work area: readers open as they always have.
    #[test]
    fn each_kind_reads_its_own_size() {
        let dir = tempfile::tempdir().unwrap();
        let settings = SettingsStore::load(dir.path().join("settings.json"));
        settings.set_pull_request_window(1440.0, 900.0).unwrap();
        settings.set_reader_window(700.0, 800.0).unwrap();
        assert_eq!((PULL_REQUEST.size)(&settings), (1440.0, 900.0));
        assert_eq!((READER.size)(&settings), (700.0, 800.0));

        settings.set_pull_request_window(300.0, 200.0).unwrap();
        assert_eq!((PULL_REQUEST.size)(&settings), PULL_REQUEST.min_size);
        assert_eq!(PULL_REQUEST.min_size, (600.0, 400.0));

        const { assert!(PULL_REQUEST.clamped_to_work_area && !READER.clamped_to_work_area) }
    }

    #[test]
    fn the_offset_anchor_and_the_detached_check_tell_the_kinds_apart() {
        let reader = reader_label("/r/specforge/file/README.md");
        let pull_request = pull_request_label(ADDRESS);

        assert!((PULL_REQUEST.owns)(&pull_request));
        assert!(!(PULL_REQUEST.owns)(&reader));
        assert!((READER.owns)(&reader));
        assert!(!(READER.owns)(&pull_request));
        assert!(is_detached_label(&reader));
        assert!(is_detached_label(&pull_request));
        assert!(!is_detached_label("main"));

        // A reader further along than the pull-request window never anchors
        // a pull-request window, nor the reverse.
        let visible = [
            (reader.as_str(), (400.0, 300.0)),
            (pull_request.as_str(), (100.0, 80.0)),
        ];
        assert_eq!(cascade_from(&PULL_REQUEST, visible), Some((124.0, 104.0)));
        assert_eq!(cascade_from(&READER, visible), Some((424.0, 324.0)));
        assert_eq!(
            cascade_from(&PULL_REQUEST, [(reader.as_str(), (400.0, 300.0))]),
            None,
            "with only readers visible, the platform places it"
        );
    }

    /// The window installs the guard the main window and readers do, so a
    /// link the webview follows itself — its native "Open Link" item — never
    /// loads another page in it; only a dev build admits the dev server.
    #[test]
    fn only_the_app_s_own_origin_loads() {
        let loads = |url: &str| allows_navigation(&tauri::Url::parse(url).unwrap());
        assert!(loads(
            "tauri://localhost/index.html?pullRequest=1&at=%2Fpr%2Fgithub%2Facme%2Fapi%2F42"
        ));
        assert!(!loads("https://github.com/acme/api/pull/42"));
        assert!(!loads("http://attacker.example/"));
        assert!(!loads("file:///etc/passwd"));
        assert_eq!(loads("http://localhost:1420/index.html"), cfg!(dev));
    }

    #[test]
    fn the_page_title_is_followed_sanitised_again() {
        assert_eq!(
            followed_title("#42 Add rate limits — acme/api", "SpecForge").as_deref(),
            Some("#42 Add rate limits — acme/api")
        );
        assert_eq!(
            followed_title(
                "#42 Add\u{202e} rate\u{200b} limits\n — acme/api",
                "SpecForge"
            )
            .as_deref(),
            Some("#42 Add rate limits — acme/api"),
            "no override, zero-width space or newline reaches the titlebar"
        );
        let overlong = format!("#42 {} — acme/api", "x".repeat(5_000));
        assert_eq!(
            followed_title(&overlong, "SpecForge")
                .unwrap()
                .chars()
                .count(),
            WINDOW_TITLE_CAP
        );
    }

    #[test]
    fn the_shell_s_own_title_and_an_empty_one_keep_the_window_s_title() {
        assert_eq!(followed_title("SpecForge", "SpecForge"), None);
        assert_eq!(followed_title("", "SpecForge"), None);
        assert_eq!(followed_title("\u{200b}\n", "SpecForge"), None);
    }

    // ---- capabilities ----

    /// The capabilities as the app is built with them.
    const DEFAULT_CAPABILITY: &str = include_str!("../capabilities/default.json");
    const PULL_REQUEST_CAPABILITY: &str = include_str!("../capabilities/pull-request.json");

    fn parse(capability: &str) -> Value {
        serde_json::from_str(capability).expect("a capability is JSON")
    }

    /// A capability's `windows` or `webviews` patterns.
    fn patterns(capability: &Value, key: &str) -> Vec<String> {
        capability[key]
            .as_array()
            .map(|patterns| {
                patterns
                    .iter()
                    .map(|pattern| pattern.as_str().expect("a pattern").to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A capability's permission identifiers, whether each is written as a
    /// string or as an object with scopes.
    fn permissions(capability: &Value) -> Vec<String> {
        capability["permissions"]
            .as_array()
            .expect("a capability lists permissions")
            .iter()
            .map(|permission| {
                permission
                    .as_str()
                    .or_else(|| permission["identifier"].as_str())
                    .expect("a permission names its identifier")
                    .to_string()
            })
            .collect()
    }

    /// Whether `pattern` matches `label` as Tauri matches a capability's
    /// windows: an exact label, or a prefix and one trailing `*`. A pattern
    /// using any other glob syntax fails the test rather than being misread.
    fn matches(pattern: &str, label: &str) -> bool {
        let prefix = pattern.strip_suffix('*');
        assert!(
            !prefix
                .unwrap_or(pattern)
                .contains(['*', '?', '[', ']', '{', '}', '\\']),
            "{pattern:?} uses glob syntax this test does not read"
        );
        match prefix {
            Some(prefix) => label.starts_with(prefix),
            None => pattern == label,
        }
    }

    #[test]
    fn the_pull_request_capability_grants_exactly_what_the_window_uses() {
        let capability = parse(PULL_REQUEST_CAPABILITY);
        assert_eq!(patterns(&capability, "windows"), ["pull-request-*"]);
        assert!(patterns(&capability, "webviews").is_empty());
        assert_eq!(
            permissions(&capability),
            [
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "core:window:allow-close",
            ]
        );
        assert!(matches("pull-request-*", &pull_request_label(ADDRESS)));
    }

    #[test]
    fn no_capability_granting_a_plugin_matches_a_pull_request_window() {
        let default = parse(DEFAULT_CAPABILITY);
        assert_eq!(patterns(&default, "windows"), ["main", "reader-*"]);
        let label = pull_request_label(ADDRESS);
        let labels = [label.clone(), format!("{label}-2")];
        for pattern in patterns(&default, "windows") {
            for label in &labels {
                assert!(!matches(&pattern, label), "default.json's {pattern:?}");
            }
        }

        for capability in [default, parse(PULL_REQUEST_CAPABILITY)] {
            let grants_a_plugin = permissions(&capability).iter().any(|permission| {
                ["dialog:", "autostart:", "notification:"]
                    .iter()
                    .any(|plugin| permission.starts_with(plugin))
            });
            if !grants_a_plugin {
                continue;
            }
            for key in ["windows", "webviews"] {
                for pattern in patterns(&capability, key) {
                    for label in &labels {
                        assert!(
                            !matches(&pattern, label),
                            "{} grants a plugin to {label} through {pattern:?}",
                            capability["identifier"]
                        );
                    }
                }
            }
        }
    }

    /// Tauri enables every capability file under `capabilities/` unless the
    /// configuration names its own, so the two files above are every
    /// capability the app has, and the checks above cover them all. A new one
    /// fails here until it is added to them. Hidden files, such as a Finder
    /// `.DS_Store`, are no capability.
    #[test]
    fn the_capabilities_are_these_two() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities");
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect();
        names.sort();
        assert_eq!(names, ["default.json", "pull-request.json"]);

        let config = parse(include_str!("../tauri.conf.json"));
        assert!(
            config["app"]["security"]["capabilities"].is_null(),
            "tauri.conf.json enables no capability of its own"
        );
    }

    /// The pull-request window's content-security policy is its page's own,
    /// installed by `main.tsx` in that window alone. A policy set by the shell
    /// would govern the main window and readers too, refusing the remote
    /// images and diagrams of the user's own documents (design D10).
    #[test]
    fn the_shell_sets_no_content_security_policy() {
        let config = parse(include_str!("../tauri.conf.json"));
        assert!(config["app"]["security"]["csp"].is_null());
        assert!(config["app"]["security"]["devCsp"].is_null());
    }
}
