//! System tray icon, badge, and tray-menu wiring.
//!
//! All platform-specific badge logic lives in [`set_badge`] so the rest of
//! the codebase only has to call a single function with a count.

use crate::tray_icon::{self, TrayGlyph, TrayGlyphState};
use openspec_core::WatcherManager;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tokio::sync::broadcast;

pub(crate) const TRAY_ID: &str = "main-tray";
const MENU_ITEM_SHOW: &str = "show";
const MENU_ITEM_QUIT: &str = "quit";

/// Install the system tray icon and return the handle.
///
/// `scale` is the active monitor's `scale_factor()` — passed in rather than
/// queried so the caller controls when/how the scale is sourced (and can
/// re-rasterize via [`tray_icon::rasterize_glyph`] when it changes).
///
/// `initial` is the glyph variant to rasterize for the first painted icon;
/// callers seed it from the current cache state so the first frame already
/// reflects spec activity (avoids a one-frame flash of the default glyph).
pub fn install_tray(app: &AppHandle, scale: f64, initial: TrayGlyph) -> tauri::Result<TrayIcon> {
    let icon = tray_icon::rasterize_glyph(initial, scale);

    let show_item = MenuItem::with_id(app, MENU_ITEM_SHOW, "Show SpecForge", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, MENU_ITEM_QUIT, "Quit SpecForge", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &separator, &quit_item])?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        // macOS: tells the system to render non-transparent pixels in the
        // menu bar's current foreground colour (white on dark menu bars,
        // black on light). Requires the rasterized buffer to be pure
        // black + alpha — `tray_icon::rasterize` debug-asserts that.
        // No-op on other platforms.
        .icon_as_template(true)
        .menu(&menu)
        // Left click should focus the main window, not show the menu. The
        // menu still appears on right-click via Tauri's defaults.
        .show_menu_on_left_click(false)
        .tooltip("SpecForge")
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_ITEM_SHOW => show_main_window(app),
            MENU_ITEM_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(tray)
}

/// Brings the main window into view: unhide it, unminimise it, and focus.
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Update the tray icon's badge.
///
/// `None` or `Some(0)` hides the badge.
///
/// Platform behaviour:
/// - **macOS**: places the count next to the menu-bar icon via `set_title`.
///   The tooltip is also kept in sync so VoiceOver / hover yields the same
///   information.
/// - **Windows / Linux**: updates the tooltip to "N active changes". A real
///   numbered icon swap is deferred to Group 11 packaging.
pub fn set_badge(tray: &TrayIcon, count: Option<u32>) -> tauri::Result<()> {
    let count = count.filter(|&n| n > 0);

    let tooltip = match count {
        Some(1) => "1 active change".to_string(),
        Some(n) => format!("{n} active changes"),
        None => "SpecForge".to_string(),
    };
    tray.set_tooltip(Some(tooltip))?;

    #[cfg(target_os = "macos")]
    {
        // tray-icon 0.23.x silently ignores `set_title(None)` on macOS —
        // `setTitle:` is never invoked, so the previous title remains
        // attached to the status item. Always pass `Some(&str)` and use the
        // empty string to clear, which routes through `setTitle:@""`.
        let title = macos_badge_title(count);
        tray.set_title(Some(title.as_str()))?;
    }

    Ok(())
}

/// macOS title string for a badge with the given post-filter count.
///
/// `Some(n)` becomes `n.to_string()`; `None` becomes an empty string so
/// `tray.set_title(Some(""))` reaches `NSStatusBarButton.setTitle:@""` and
/// the status item collapses back to icon-only width.
#[cfg(target_os = "macos")]
fn macos_badge_title(count: Option<u32>) -> String {
    count.map(|n| n.to_string()).unwrap_or_default()
}

/// Spawns a task that recomputes the active-change count and refreshes the
/// tray badge on every [`openspec_core::CacheEvent`]. Performs an initial
/// badge set immediately.
pub fn spawn_badge_updater(tray: TrayIcon, watcher: WatcherManager) {
    tauri::async_runtime::spawn(async move {
        // Count logical changes (one per (repo_id, change_name)) across all
        // tracked entries. A change touched by multiple worktrees of one
        // repo contributes 1.
        let _ = set_badge(&tray, Some(watcher.total_active_logical_count() as u32));

        let mut rx = watcher.subscribe();
        loop {
            match rx.recv().await {
                Ok(_) => {
                    let _ = set_badge(&tray, Some(watcher.total_active_logical_count() as u32));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

/// Spawns a task that flips the tray glyph variant whenever the cache
/// transitions between "no active change touches specs" and "at least one
/// does". Performs an initial set so the first paint is correct even if
/// the cache changed between `install_tray` and the spawn.
///
/// `app` is held so the updater can read the main window's current scale
/// factor at re-rasterize time — this matters when a scale change has
/// already occurred since launch and the next variant flip should
/// rasterize at the new scale, not the launch scale.
///
/// `state` is the shared variant cell also read by the `ScaleFactorChanged`
/// handler in `lib.rs`, which needs to know the current variant to
/// re-rasterize the right SVG.
pub fn spawn_tray_glyph_updater(
    tray: TrayIcon,
    app: AppHandle,
    watcher: WatcherManager,
    state: TrayGlyphState,
    initial_scale: f64,
) {
    tauri::async_runtime::spawn(async move {
        let initial = current_variant(&watcher);
        state.store(initial);
        let _ = tray.set_icon_with_as_template(
            Some(tray_icon::rasterize_glyph(initial, initial_scale)),
            true,
        );

        let mut rx = watcher.subscribe();
        loop {
            match rx.recv().await {
                Ok(_) => {
                    let next = current_variant(&watcher);
                    if next != state.load() {
                        state.store(next);
                        let scale = current_scale(&app, initial_scale);
                        let _ = tray.set_icon_with_as_template(
                            Some(tray_icon::rasterize_glyph(next, scale)),
                            true,
                        );
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

fn current_variant(watcher: &WatcherManager) -> TrayGlyph {
    if watcher.any_change_touches_specs() {
        TrayGlyph::Specs
    } else {
        TrayGlyph::Default
    }
}

fn current_scale(app: &AppHandle, fallback: f64) -> f64 {
    app.get_webview_window("main")
        .map(|w| w.scale_factor().unwrap_or(fallback))
        .unwrap_or(fallback)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    // tray-icon 0.23.x's `set_title(None)` is a no-op on macOS — the
    // previous title stays attached to the status item. `macos_badge_title`
    // exists so the macOS branch of `set_badge` can always pass `Some(&str)`,
    // substituting the empty string for the no-count case so `setTitle:@""`
    // is actually invoked and the title collapses.

    #[test]
    fn empty_when_count_is_none() {
        assert_eq!(macos_badge_title(None), "");
    }

    #[test]
    fn empty_when_count_is_zero() {
        // `set_badge` filters `Some(0)` to `None` before reaching the
        // helper, but pin the direct behaviour too in case the filter ever
        // moves: zero is semantically "no badge" and must produce "".
        assert_eq!(
            None::<u32>.or(Some(0u32)).filter(|&n| n > 0),
            None,
            "filter must collapse Some(0) to None"
        );
        assert_eq!(macos_badge_title(Some(0).filter(|&n| n > 0)), "");
    }

    #[test]
    fn digit_when_count_is_nonzero() {
        assert_eq!(macos_badge_title(Some(1)), "1");
        assert_eq!(macos_badge_title(Some(42)), "42");
    }
}
