# Tasks

## 1. Build the custom macOS application menu

- [x] 1.1 Add a menu-construction helper to the Tauri shell (`crates/specforge/src/menu.rs`) exposing `build_app_menu(handle: &AppHandle<R>) -> tauri::Result<Menu<R>>`, gated `#[cfg(target_os = "macos")]`
- [x] 1.2 Build the App submenu ("SpecForge"): enriched `about` · separator · `services` · separator · `hide` / `hide_others` / `show_all` · separator · `quit`
- [x] 1.3 Build the Edit submenu: `undo` / `redo` · separator · `cut` / `copy` / `paste` / `select_all`
- [x] 1.4 Build the Window submenu with `Submenu::with_id_and_items(.., WINDOW_SUBMENU_ID, ..)` so macOS attaches the Windows-menu role: `minimize` / `maximize` · separator · `close_window`
- [x] 1.5 Compose the three submenus into a `Menu` and return it

## 2. Populate the About metadata

The native macOS panel renders only `name`/`version`/`short_version`/`copyright`/`icon`/`credits`, so all prose content rides in `credits`.

- [x] 2.1 Construct `AboutMetadata` via `AboutMetadataBuilder` with `name = "SpecForge"` (literal display name, not the crate name)
- [x] 2.2 Set `version` from `handle.package_info().version.to_string()`
- [x] 2.3 Set `copyright = "© 2026 Avant Media Ltd"` (mirror `bundle.copyright`; comment cross-references the bundle value)
- [x] 2.4 Set `credits` to a multi-line block: tagline (names the OpenSpec format, harmonised with `bundle.shortDescription`) + canonical repo URL + an `MIT License` line
- [x] 2.5 Omit `icon` (the macOS native panel uses the bundle icon)
- [x] 2.6 Do NOT set `comments` / `website` / `website_label` / `license` / `authors` — the native macOS panel ignores them and the module is macOS-only, so they would be dead, misleading code

## 3. Install the menu on macOS only

- [x] 3.1 In `crates/specforge/src/lib.rs` `run()`, install the menu via `app.handle().set_menu(..)` in `setup`, under `#[cfg(target_os = "macos")]`
- [x] 3.2 Confirm non-macOS builds compile unchanged and install no custom menu (module + call site both `#[cfg(target_os = "macos")]`, mirroring `dock_badge`; Linux CI confirms)
- [x] 3.3 Confirm the menu install does not disturb the existing `run()` ordering (added as a self-contained block before config/event/cache setup; forwarder still installed before cache population)

## 4. Fix the stale repository URL

- [x] 4.1 In `crates/specforge/tauri.conf.json`, correct `bundle.homepage` from `https://github.com/avantmedia/specforge` to the canonical `https://github.com/avantmedialtd/specforge`
- [x] 4.2 Confirm the repo URL in the `credits` text and `bundle.homepage` now agree

## 5. Build and verify

- [x] 5.1 Run `cargo check --workspace` and confirm it passes
- [x] 5.2 Run `bun run build` (frontend `tsc --noEmit && vite build`) and confirm it passes (no frontend change expected, but verify nothing broke)
- [x] 5.3 Run `bun tauri dev`; open the SpecForge app menu → "About SpecForge" and confirm the panel shows the name, version `0.1.0`, the copyright line, and a credits block containing the tagline, the repo URL, and an `MIT License` line — verified visually in the running worktree build
- [x] 5.4 Confirm the credits text shows `github.com/avantmedialtd/specforge` rendered as text (the native panel does not hyperlink it) — verified
- [x] 5.5 In the running app, focus the workspace-rename field in Settings and confirm `Cmd-C` / `Cmd-V` / `Cmd-X` / `Cmd-A` work (Edit submenu preserved) — verified
- [x] 5.6 Confirm `Cmd-M` minimizes, and the Window menu shows the system-managed items (Zoom, Bring All to Front) — i.e. the Windows-menu role is attached, not lost — verified

## 6. Capture the spec deltas

- [x] 6.1 Confirm the new `application-menu` capability spec matches the implementation: App submenu first item About; Edit undo/redo/cut/copy/paste/select-all; Window registered via `WINDOW_SUBMENU_ID`; About renders name/version/copyright + credits (tagline/repo URL/MIT); version read at runtime; repo URL == `bundle.homepage`; macOS-gated
- [x] 6.2 Confirm the `product-identity` delta's About-panel scenarios match the shipped copy: name "SpecForge", version, copyright "© 2026 Avant Media Ltd"; credits tagline "A menu-bar viewer for OpenSpec changes across your workspaces." names the OpenSpec format and tracks `bundle.shortDescription`
