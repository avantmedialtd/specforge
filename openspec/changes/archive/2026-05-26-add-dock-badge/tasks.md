# Tasks

## 1. New Rust module for the Dock-badge updater

- [x] 1.1 Create `crates/specforge/src/dock_badge.rs`. File-level `#[cfg(target_os = "macos")]` so non-macOS builds do not compile this module at all. Top-level doc comment explaining: macOS-only Dock-tile badge driven by the same `WatcherManager` event stream as the tray badge; all updates funnelled through `set_dock_badge` so future workarounds (regressed `Window::set_badge_count` behaviour, stale-digit-on-clear, etc.) live in one place.
- [x] 1.2 In `crates/specforge/src/lib.rs`, add `#[cfg(target_os = "macos")] mod dock_badge;` alongside the other module declarations near the top of the file.

## 2. Updater function and funnel helper

- [x] 2.1 In `dock_badge.rs`, define `pub fn set_dock_badge(window: &Window, count: Option<u32>) -> tauri::Result<()>`. Filter `Some(0)` to `None` first (mirroring the tray badge's filter), then call `window.set_badge_count(count.map(|n| n as i64))`. The funnel rationale lives in the doc comment.
- [x] 2.2 In the same file, define `pub fn spawn_dock_badge_updater(window: Window, watcher: WatcherManager)`. Mirror `tray::spawn_badge_updater` structurally: `tauri::async_runtime::spawn` a task that (a) calls `set_dock_badge(&window, Some(watcher.total_active_logical_count() as u32))` once at start, then (b) loops over `watcher.subscribe()`, recomputing and reapplying on every `CacheEvent::*`. Handle `RecvError::Lagged` with `continue`, `RecvError::Closed` with `return`.
- [x] 2.3 Confirm `cargo check -p specforge` passes on macOS.
- [x] 2.4 Confirm a non-macOS build still compiles. If you do not have a cross-build available, at minimum run `cargo check -p specforge --target x86_64-unknown-linux-gnu` if that target is installed; otherwise note in the verification step that this requires CI to confirm.

## 3. Wire into the app startup

- [x] 3.1 In `crates/specforge/src/lib.rs`, inside the existing `if let Some(main_window) = app.get_webview_window("main")` block, add a `#[cfg(target_os = "macos")]` call to `dock_badge::spawn_dock_badge_updater(main_window.clone(), watcher.clone())`.
- [x] 3.2 Place the call after the vibrancy block (the existing macOS-specific block) and before the `on_window_event` handler — keeps the macOS-specific setup grouped and matches the cfg placement style already used in this file.
- [x] 3.3 Confirm the initial badge is set after the cache has been populated. (The synchronous `block_on` populate runs above, in `setup`, before the main-window block — so by the time the updater spawns, `total_active_logical_count()` already reflects the registered workspaces. The updater's first call is therefore correct from the start.)

## 4. Verification

- [x] 4.1 Run `cargo test` — workspace tests must still pass. No new Rust tests are required: `spawn_dock_badge_updater` is a thin pass-through to `WatcherManager` and `Window::set_badge_count`.
- [x] 4.2 Start `bun tauri dev` with a workspace registered that has exactly three non-archived logical changes. Confirm: a red badge with "3" is visible on the SpecForge Dock tile, and the menu-bar tray badge shows the same "3".
- [x] 4.3 Archive one change (`git mv openspec/changes/<id> openspec/changes/archive/` inside a tracked workspace). Confirm the Dock badge updates to "2" within the watcher debounce window. Confirm the tray badge updates to "2" at the same time.
- [x] 4.4 Archive the remaining two changes. Confirm the Dock badge disappears entirely — i.e. `set_badge_count(None)` actually clears the tile. **If a stale digit remains**, capture the exact failure mode and add a workaround inside `set_dock_badge` only — likely an `objc2` call to set the dock tile's badge label to an empty `NSString`. The funnel helper is specifically where this kind of workaround belongs.
- [x] 4.5 Add a new change directory back to a tracked workspace (e.g. `mkdir openspec/changes/test-change-1 && touch openspec/changes/test-change-1/proposal.md`). Confirm the Dock badge transitions from no-badge to "1" within the watcher debounce window.
- [x] 4.6 Hold Cmd and press Tab. Confirm the SpecForge tile in the app switcher displays the same badge digit as the Dock tile.
- [x] 4.7 Close the main window (close button hides it). Add another change directory on disk. Confirm the Dock badge updates from "1" to "2" without the main window being visible.
- [x] 4.8 Quit SpecForge entirely (Cmd-Q). Relaunch. With the same workspace and the same set of active changes, confirm the Dock badge is present on first paint — no flash of empty tile.
- [x] 4.9 Throughout 4.2 – 4.8, confirm the tray badge and Dock badge always show the same number. They MUST NOT drift.

## 5. Spec validation

- [x] 5.1 Run `openspec validate add-dock-badge` and confirm the change passes validation.
