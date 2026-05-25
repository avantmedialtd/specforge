# Preserve Tray Icon Template Flag Across Updates

## Why

The macOS template-image flag (`NSImage.isTemplate`) is a property of each individual `NSImage`, not of the tray. `TrayIconBuilder::icon_as_template(true)` only sets the flag on the **initial** icon. Every subsequent `tray.set_icon(...)` call hands the system a fresh `NSImage` whose flag defaults to `false`, so macOS renders the buffer's literal pixels (pure black) instead of recolouring it to the menu-bar's foreground colour. In dark mode this makes the tray icon appear black on black — effectively invisible.

The bug was latent until the spec-activity glyph variant landed: previously the only `set_icon` site was the `ScaleFactorChanged` handler, which fires rarely. The new glyph updater's "initial set" calls `set_icon` at startup, so the flag is stripped on every launch.

## What Changes

- Replace every `tray.set_icon(Some(...))` call with `tray.set_icon_with_as_template(Some(...), true)`, which atomically sets both the icon and the template flag in one operation. Per Tauri's own documentation on `set_icon_with_as_template`, this avoids the visible flicker that would happen if `set_icon` were followed by a separate `set_icon_as_template(true)`.
- Three call sites: the glyph updater's initial set, the variant flip on `CacheEvent`, and the `ScaleFactorChanged` handler.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tray-indicator`: Adds a new requirement asserting that the tray glyph SHALL be rendered as a template image on every set, not only on the initial install. This codifies the invariant so a future contributor can't reintroduce the bug by reaching for `set_icon` directly.

## Impact

- `crates/specforge/src/tray.rs`: two call sites in `spawn_tray_glyph_updater` switched to `set_icon_with_as_template`.
- `crates/specforge/src/lib.rs`: one call site in the `ScaleFactorChanged` arm switched to `set_icon_with_as_template`.
- The builder-level `icon_as_template(true)` at `tray.rs:47` remains — it still correctly configures the initial install. Optional cleanup, not strictly required.
- No new dependencies. No IPC changes. No frontend changes.
