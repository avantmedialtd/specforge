# SVG-source the tray icon

## Why

Today the tray icon is a fixed-resolution PNG handed to Tauri via `app.default_window_icon()` (see `crates/specforge/src/tray.rs:19-23`). That works, but it bakes in three frictions:

1. **Drift between source and product**: any future icon iteration requires re-exporting a stack of PNGs (@1x, @2x) and remembering to refresh the rasters in `crates/specforge/icons/`. The "source of truth" is whatever lives in the designer's file, not anything the repo can point at.
2. **HiDPI ceiling**: a pre-rendered @2x PNG is crisp at 2x but slightly soft on a 3x retina panel and a touch lossy on the rare 1.5x Windows scale. Rasterizing at the active `scale_factor()` removes the ceiling.
3. **Tray vs app icon coupling**: the tray reuses the *window* icon, which is a different design problem — the menu-bar icon wants a small monochrome silhouette rendered as a template; the dock / installer icon wants colourful 1024px artwork. Splitting them frees both.

## What Changes

- Add `crates/specforge/icons/tray-icon.svg` as the source of truth for the tray glyph (monochrome, intended for macOS template rendering).
- Add `resvg` + `usvg` to the `specforge` crate (pure-Rust, no system libs).
- In `install_tray`, replace `app.default_window_icon()` with a helper that rasterizes the bundled SVG at the size and scale appropriate for the active monitor and returns a `tauri::image::Image`.
- Re-rasterize and call `tray.set_icon(...)` when the main window's monitor changes scale factor.
- App / dock / installer icons (`icon.icns`, `icon.ico`, the sized PNGs) are **out of scope**: they keep their existing raster sources, generated as before by `tauri icon`.

## Capabilities

### Modified Capabilities

- `tray-indicator`: gains a sharpness/scaling requirement — the icon is rendered at the active monitor's pixel density rather than a fixed pre-rendered resolution. All existing requirements (presence, badge, click-to-focus, notifications) carry over unchanged.

### New Capabilities

(none)

## Impact

- **Code**: `crates/specforge/src/tray.rs` (icon loading swap, scale-change handler), new `crates/specforge/src/tray_icon.rs` (rasterizer). New `Cargo.toml` deps: `resvg`, `usvg`.
- **Assets**: new `crates/specforge/icons/tray-icon.svg`. Existing PNG icon set stays — still used for window, dock, installer.
- **Runtime cost**: one in-process SVG rasterization at startup (~ms for a 22pt glyph) plus one per scale-factor change. No background work.
- **Binary size**: `resvg` + `usvg` add ~hundreds of KB; acceptable for a desktop app.
- **Risk**: macOS template rendering requires the rasterized buffer to be pure black + alpha. The SVG must be authored as a solid-black silhouette, not full-colour artwork. A startup sanity check guards against drift (see `design.md`).
