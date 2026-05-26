# Add Dock Badge with Active Change Count (macOS)

## Why

The menu-bar tray icon already shows a numeric badge for the count of active logical changes across all registered workspaces. When the user is glancing at the Dock or scanning CMD+Tab, that count is invisible — the OpenSpec activity signal is available in only one surface.

A Dock badge mirrors the count onto the other always-visible macOS surface and surfaces it in the CMD+Tab switcher for free (the switcher renders the same Dock tile). The user can then see at a glance, from anywhere in their workspace, how many in-flight changes exist across all registered workspaces — even when SpecForge is in the background, even when the main window is hidden.

A small spike (a hardcoded `Window::set_badge_count(Some(3))` in `lib.rs::setup`) confirmed that the call paints correctly on the current Tauri 2 pin in this repo. The unresolved upstream report at [tauri#13905](https://github.com/tauri-apps/tauri/issues/13905) does not reproduce here, so no `objc2`-direct fallback is required for v1.

## What Changes

- A Dock tile badge SHALL display the same `total_active_logical_count()` value as the menu-bar tray badge, updating on every `CacheEvent` from `WatcherManager`.
- The badge SHALL be hidden when the count is zero.
- The Dock and tray badges MUST never drift apart — both consume the same broadcast stream.
- The CMD+Tab application switcher inherits the badge automatically because macOS renders the same Dock tile in both surfaces.
- The implementation is macOS-only. Windows and Linux equivalents (overlay icons, Unity launcher) are deferred to a future change.
- There is no settings toggle. The Dock badge mirrors the tray badge unconditionally.

## Capabilities

### New Capabilities
- `dock-indicator`: defines the macOS Dock-tile badge presence and lifecycle, distinct from the menu-bar `tray-indicator`.

### Modified Capabilities
<!-- none -->

## Impact

- **Rust shell (`crates/specforge/src/`)**: new module `dock_badge.rs` (file-level `#[cfg(target_os = "macos")]`) exposing `spawn_dock_badge_updater(window: Window, watcher: WatcherManager)`. Mirrors the structure of `tray::spawn_badge_updater`: subscribe to the broadcast, recompute the count on every event, route every set through a single `set_dock_badge` helper that calls `Window::set_badge_count`.
- **Rust shell (`crates/specforge/src/lib.rs`)**: one wiring call inside the existing `if let Some(main_window) = app.get_webview_window("main")` block, gated by `#[cfg(target_os = "macos")]`. Module declaration also macOS-gated.
- **No `openspec-core` changes.** The existing `WatcherManager` broadcast and `total_active_logical_count()` already supply everything the updater needs.
- **No frontend changes.** No IPC, no React, no settings field.
- **`tray-indicator` capability untouched.** The tray badge updater continues to do its own thing; the Dock updater is an independent subscriber to the same broadcast.
