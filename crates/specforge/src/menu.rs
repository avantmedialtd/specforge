//! macOS application menu.
//!
//! Tauri installs a default application menu on macOS whenever the app sets no
//! menu of its own. That default carries an "About SpecForge" item, but it
//! opens the bare system panel (name + version only). This module installs a
//! custom menu so the About item can be enriched, and so the standard Edit and
//! Window submenus are preserved.
//!
//! The native macOS About panel has a narrow field set: muda renders it via
//! `NSApplication::orderFrontStandardAboutPanelWithOptions:`, which reads ONLY
//! `name`, `version`, `short_version`, `copyright`, `icon`, and `credits` — it
//! ignores `comments`, `website`, `license`, and `authors`. So the tagline, the
//! repository URL, and the license line are folded into `credits`, the one
//! rich-text field it does show. `credits` renders as a plain
//! `NSAttributedString`, so the URL appears as text, not a clickable link.
//!
//! Owning the menu discards Tauri's auto-default, so this module also rebuilds
//! the standard Edit (cut/copy/paste/select-all/undo/redo) and Window submenus —
//! otherwise those system shortcuts stop working in the app's text inputs (e.g.
//! the workspace-rename field in Settings). The Window submenu is built with
//! [`WINDOW_SUBMENU_ID`] so Tauri attaches the macOS Windows-menu role (Zoom,
//! Bring All to Front, the live window list, Cmd-` cycling) via `setWindowsMenu:`;
//! without that id the role is never applied.
//!
//! macOS-only: on Windows/Linux a custom `Menu` renders as a visible window menu
//! bar, which is inappropriate for a tray-resident app. The whole module is
//! `#[cfg(target_os = "macos")]`-gated at its declaration in `lib.rs`, and the
//! caller installs the menu under the same gate.

use crate::events::{EVENT_TOGGLE_COMMIT_RAIL, EVENT_TOGGLE_SIDEBAR};
use tauri::{
    menu::{
        AboutMetadataBuilder, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
        SubmenuBuilder, WINDOW_SUBMENU_ID,
    },
    AppHandle, Emitter, Manager, Runtime,
};

/// Canonical SpecForge repository, shown in the About panel's credits text.
/// Kept in agreement with `bundle.homepage` in `tauri.conf.json` (both point
/// at the `avantmedialtd` org that the configured git remote uses).
const REPOSITORY_URL: &str = "https://github.com/avantmedialtd/specforge";

/// Build the macOS application menu: an enriched App submenu (carrying the
/// About item) plus the standard Edit and Window submenus.
pub fn build_app_menu<R: Runtime>(handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    // Read the version at runtime from the package metadata so it can never
    // drift from the shipped bundle version.
    let version = handle.package_info().version.to_string();

    // The native macOS panel only renders name/version/copyright/icon/credits
    // (see the module docs), so the tagline, repository URL, and license are
    // folded into `credits`. The leading line is the tagline, harmonised with
    // `bundle.shortDescription`, and names the OpenSpec format SpecForge reads
    // (format sense, not a product name).
    let credits = format!(
        "A menu-bar viewer for OpenSpec changes across your workspaces.\n\n{REPOSITORY_URL}\nMIT License",
    );

    let about_metadata = AboutMetadataBuilder::new()
        // Literal display name — `package_info().name` is the lowercase crate
        // name (`specforge`), which is wrong for a user-facing panel.
        .name(Some("SpecForge"))
        .version(Some(version))
        // Mirrors `bundle.copyright` in tauri.conf.json — keep the two in sync.
        .copyright(Some("© 2026 Avant Media Ltd"))
        .credits(Some(credits))
        // Icon omitted: the macOS native About panel uses the bundle icon.
        // comments/website/license/authors are deliberately not set — macOS's
        // native panel ignores them (their content lives in `credits` above).
        .build();

    // App submenu. macOS replaces the title with the bundle name, but a
    // descriptive title keeps the intent clear in code.
    let app_menu = SubmenuBuilder::new(handle, "SpecForge")
        .about(Some(about_metadata))
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    // Edit submenu — rebuilt so Cmd-Z/Cmd-X/Cmd-C/Cmd-V/Cmd-A keep working in
    // app text inputs after we replace the auto-default menu.
    let edit_menu = SubmenuBuilder::new(handle, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // View submenu — the pane-visibility toggles (`application-menu`: View
    // Submenu Pane Toggles). Item ids double as the Tauri event names emitted
    // to the webview, so `handle_menu_event` needs no id→event mapping. The
    // accelerators live HERE and only here on macOS: the frontend registers
    // its own keydown handler for the same combos exclusively on surfaces
    // without this menu (web, Windows/Linux), so one keypress always reaches
    // exactly one handler.
    let view_menu = SubmenuBuilder::new(handle, "View")
        .item(&MenuItem::with_id(
            handle,
            EVENT_TOGGLE_SIDEBAR,
            "Toggle Sidebar",
            true,
            Some("CmdOrCtrl+B"),
        )?)
        .item(&MenuItem::with_id(
            handle,
            EVENT_TOGGLE_COMMIT_RAIL,
            "Toggle Commit Rail",
            true,
            Some("Alt+CmdOrCtrl+B"),
        )?)
        .build()?;

    // Window submenu — built with WINDOW_SUBMENU_ID so Tauri registers it as
    // the macOS Windows menu (setWindowsMenu:), restoring Zoom, Bring All to
    // Front, the live window list, and Cmd-` cycling. Mirrors Tauri's own
    // default Window submenu. Cmd-W (close_window) issues a close request to
    // the focused window and deliberately does NOT branch on which window that
    // is: the main window's CloseRequested handler in lib.rs intercepts it and
    // hides, matching the traffic-light close button, while a reader window
    // installs no such handler and is destroyed. One item, two correct
    // behaviours, each decided by the window that receives the request rather
    // than duplicated here where the copy could silently disagree.
    let window_menu = Submenu::with_id_and_items(
        handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::close_window(handle, None)?,
        ],
    )?;

    Menu::with_items(handle, &[&app_menu, &edit_menu, &view_menu, &window_menu])
}

/// Handle activation of a View-submenu item: show and focus the main window
/// first (a toggle aimed at a hidden window would otherwise do its work
/// invisibly), then emit the pane-toggle event the item id names. Every other
/// menu item falls through untouched — the predefined App/Edit/Window items
/// are handled natively by macOS and never reach this path with a matching id.
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) {
    let id = event.id().as_ref();
    if id != EVENT_TOGGLE_SIDEBAR && id != EVENT_TOGGLE_COMMIT_RAIL {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit(id, ());
}
