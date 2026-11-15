//! The native window chrome follows the stored scheme (`theme-system`: *The
//! Scheme Is a Selectable Preference*; design D10).
//!
//! The page resolves the scheme for itself (`src/appearance.ts` stamps
//! `data-scheme`), but titlebars, native menus and system controls belong to
//! the window, not the page. An explicit Light or Dark is therefore handed to
//! the window too, and System clears the override so the operating system's
//! own setting governs again. On macOS and Linux the theme is application-wide
//! whichever call sets it; on Windows it is per window, which is why every
//! open window is visited and every new one is built with it.

use openspec_app::ColorScheme;
use tauri::{AppHandle, Manager, Theme};

/// The native theme for a stored scheme: forced for Light and Dark, `None` —
/// follow the operating system — for System.
pub fn native_theme(scheme: ColorScheme) -> Option<Theme> {
    match scheme {
        ColorScheme::System => None,
        ColorScheme::Light => Some(Theme::Light),
        ColorScheme::Dark => Some(Theme::Dark),
    }
}

/// Apply `scheme` to the application and to every window open now. A window
/// opened later takes it from its builder (`reader::open_detached_labelled`),
/// so the two together cover every window there will be.
pub fn apply_native_theme(app: &AppHandle, scheme: ColorScheme) {
    let theme = native_theme(scheme);
    app.set_theme(theme);
    for window in app.webview_windows().values() {
        let _ = window.set_theme(theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// System hands the choice back to the operating system; the two explicit
    /// schemes force the matching native theme.
    #[test]
    fn each_scheme_maps_to_its_native_theme() {
        assert_eq!(native_theme(ColorScheme::System), None);
        assert_eq!(native_theme(ColorScheme::Light), Some(Theme::Light));
        assert_eq!(native_theme(ColorScheme::Dark), Some(Theme::Dark));
    }
}
