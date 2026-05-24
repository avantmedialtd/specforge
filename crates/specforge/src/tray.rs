//! System tray icon, badge, and tray-menu wiring.
//!
//! All platform-specific badge logic lives in [`set_badge`] so the rest of
//! the codebase only has to call a single function with a count.

use openspec_core::WatcherManager;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tokio::sync::broadcast;

const TRAY_ID: &str = "main-tray";
const MENU_ITEM_SHOW: &str = "show";
const MENU_ITEM_QUIT: &str = "quit";

/// Install the system tray icon and return the handle.
pub fn install_tray(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon must be configured in tauri.conf.json");

    let show_item = MenuItem::with_id(app, MENU_ITEM_SHOW, "Show SpecForge", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, MENU_ITEM_QUIT, "Quit SpecForge", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &separator, &quit_item])?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        // macOS: tells the system to render non-transparent pixels in the
        // menu bar's current foreground colour (white on dark menu bars,
        // black on light). The placeholder icon is a near-black rounded
        // square on transparent background, which renders correctly under
        // this treatment. No-op on other platforms.
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
        let title = count.map(|n| n.to_string());
        tray.set_title(title.as_deref())?;
    }

    Ok(())
}

/// Spawns a task that recomputes the active-change count and refreshes the
/// tray badge on every [`openspec_core::CacheEvent`]. Performs an initial
/// badge set immediately.
pub fn spawn_badge_updater(tray: TrayIcon, watcher: WatcherManager) {
    tauri::async_runtime::spawn(async move {
        let _ = set_badge(&tray, Some(watcher.total_active_count() as u32));

        let mut rx = watcher.subscribe();
        loop {
            match rx.recv().await {
                Ok(_) => {
                    let _ = set_badge(&tray, Some(watcher.total_active_count() as u32));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}
