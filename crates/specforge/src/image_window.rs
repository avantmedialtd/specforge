//! The zoom window: one image file's versions apart from the main window
//! (`diff-view`: *Image Comparison*, Zooming; design D11).
//!
//! The detached-window helper's third kind (see `reader.rs`, whose machinery
//! it shares): a window labelled `image-<hash>` that loads
//! `index.html?imageWindow=1&at=<address>`, focused rather than opened twice,
//! with a native titlebar and no `CloseRequested` handler, so closing it
//! destroys it. What sets it apart:
//!
//! - **Size.** It opens at one fixed size, clamped to the work area of the
//!   launching window's monitor, and remembers none: a zoom window is looked
//!   at and closed, and fits to whatever size it is given.
//! - **Title.** It names the file, whose path a pull request's author may
//!   have chosen, so it passes through `openspec_app::sanitize_window_title`
//!   when the window is built and follows the page's title as a pull-request
//!   window's does.
//! - **Permissions.** `capabilities/image.json` grants closing itself and
//!   nothing else. Its content-security policy is its page's own, the
//!   pull-request window's, installed by `main.tsx` before the root renders.

use std::sync::Mutex;

use openspec_app::{sanitize_window_title, SettingsStore};
use tauri::{AppHandle, Window};

use crate::pull_request_window::follow_document_title;
use crate::reader::{open_detached, short_hash, DetachedKind};

/// Every zoom window's label starts with this. Its own capability matches
/// it, `default.json` never does, and the window-state plugin skips it.
pub const IMAGE_LABEL_PREFIX: &str = "image-";

/// The size every zoom window opens at, before the work area clamps it.
pub const IMAGE_WINDOW_SIZE: (f64, f64) = (1200.0, 820.0);

/// The smallest a zoom window can be resized to.
pub const IMAGE_WINDOW_MIN_SIZE: (f64, f64) = (480.0, 360.0);

/// The zoom window, the detached-window helper's third kind.
pub const IMAGE_WINDOW: DetachedKind = DetachedKind {
    label: image_label,
    owns: is_image_label,
    query_flag: "imageWindow",
    min_size: IMAGE_WINDOW_MIN_SIZE,
    size: image_size,
    clamped_to_work_area: true,
};

/// The window label for the zoom window of `address`, an `imageWindowAddress`
/// result. The browser skin names the file's tab from the same hash.
pub fn image_label(address: &str) -> String {
    format!("{IMAGE_LABEL_PREFIX}{}", short_hash(address))
}

/// Whether `label` names a zoom window.
pub fn is_image_label(label: &str) -> bool {
    label.starts_with(IMAGE_LABEL_PREFIX)
}

fn image_size(_settings: &SettingsStore) -> (f64, f64) {
    IMAGE_WINDOW_SIZE
}

/// Held from looking for the open window until a new one is built, for the
/// reason `pull_request_window`'s is: the command is async, so two quick asks
/// for one file run side by side.
static OPENING: Mutex<()> = Mutex::new(());

/// Open — or focus — the zoom window for `address`, asked for by the window
/// `launcher`, whose monitor's work area a new window fits. `title` is the
/// frontend's `imageWindowTitle`, sanitised here all the same.
pub fn open_image_window(
    app: &AppHandle,
    launcher: &Window,
    address: &str,
    title: &str,
) -> tauri::Result<()> {
    let _opening = OPENING.lock().unwrap_or_else(|e| e.into_inner());
    open_detached(
        app,
        &IMAGE_WINDOW,
        address,
        &sanitize_window_title(title),
        Some(launcher),
        |builder| builder.on_document_title_changed(follow_document_title),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pull_request_window::{pull_request_label, PULL_REQUEST};
    use crate::reader::{is_detached_label, reader_label, READER};

    const ADDRESS: &str = r#"{"kind":"commit","repoId":"/r/.git","sha":"c7701f2","path":"icons/app.png","oldPath":null,"sides":{"old":"4c5774c","new":"c7701f2"}}"#;

    #[test]
    fn the_label_and_url_name_the_address() {
        let label = image_label(ADDRESS);
        assert_eq!(label, format!("image-{}", short_hash(ADDRESS)));
        assert_eq!((IMAGE_WINDOW.label)(ADDRESS), label);
        assert_ne!(label, image_label(&ADDRESS.replace("app.png", "new.png")));

        // The page reads back the flag `windowKind` selects the root by, and
        // the address itself, whatever JSON it carries.
        let page =
            tauri::Url::parse(&format!("tauri://localhost/{}", IMAGE_WINDOW.url(ADDRESS))).unwrap();
        let query: Vec<(String, String)> = page.query_pairs().into_owned().collect();
        assert_eq!(
            query,
            [
                ("imageWindow".to_string(), "1".to_string()),
                ("at".to_string(), ADDRESS.to_string()),
            ]
        );
    }

    #[test]
    fn labels_use_only_characters_tauri_accepts() {
        for address in [ADDRESS, "", "{\"path\":\"a b/ç.png\"}"] {
            let label = image_label(address);
            for label in [label.clone(), format!("{label}-2")] {
                assert!(
                    label
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_')),
                    "label {label:?} contains a character Tauri rejects"
                );
                assert!(is_image_label(&label));
            }
        }
    }

    /// One fixed size, clamped to the work area, never the remembered sizes
    /// of the other two kinds.
    #[test]
    fn it_opens_at_its_own_fixed_size() {
        let dir = tempfile::tempdir().unwrap();
        let settings = SettingsStore::load(dir.path().join("settings.json"));
        settings.set_pull_request_window(1440.0, 900.0).unwrap();
        settings.set_reader_window(700.0, 800.0).unwrap();
        assert_eq!((IMAGE_WINDOW.size)(&settings), (1200.0, 820.0));
        assert_eq!(IMAGE_WINDOW.min_size, (480.0, 360.0));
        const { assert!(IMAGE_WINDOW.clamped_to_work_area) }
    }

    #[test]
    fn the_kinds_tell_their_labels_apart() {
        let image = image_label(ADDRESS);
        let reader = reader_label("/r/specforge/file/README.md");
        let pull_request = pull_request_label("/pr/github/acme/api/42");
        assert!((IMAGE_WINDOW.owns)(&image));
        assert!(!(IMAGE_WINDOW.owns)(&reader));
        assert!(!(IMAGE_WINDOW.owns)(&pull_request));
        assert!(!(READER.owns)(&image));
        assert!(!(PULL_REQUEST.owns)(&image));
        assert!(
            is_detached_label(&image),
            "the window-state plugin skips it"
        );
        assert!(!is_image_label("main"));
    }
}
