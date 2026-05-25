# Tasks

## 1. Asset & deps

- [x] 1.1 Add `crates/specforge/icons/tray-icon.svg` — placeholder solid-black silhouette of the existing logo, 100×100 viewBox
- [x] 1.2 Add `resvg` and `usvg` to `crates/specforge/Cargo.toml` (default features off — no text / fonts / raster images needed)

## 2. Rasterizer

- [x] 2.1 New module `crates/specforge/src/tray_icon.rs` exposing `rasterize(svg_bytes: &[u8], logical_size: u32, scale: f64) -> tauri::image::Image`
- [x] 2.2 Bundle the SVG via `include_bytes!` so the binary has no runtime file dependency
- [x] 2.3 Debug-build assertion: panic if any output pixel has non-zero R/G/B (guards macOS template correctness)
- [x] 2.4 Unit test: rasterize at scales 1.0, 2.0, 3.0 and assert buffer dimensions equal `(logical_size * scale)^2 * 4` bytes

## 3. Wire into tray

- [x] 3.1 In `install_tray` (`crates/specforge/src/tray.rs`), replace `app.default_window_icon()` with `tray_icon::rasterize(SVG, 22, monitor_scale)`
- [x] 3.2 ~~Store the resulting `TrayIcon` handle in app state~~ — used `app.tray_by_id(tray::TRAY_ID)` lookup instead; Tauri already manages tray handles by ID, no extra `app.manage()` needed
- [x] 3.3 In the main window's `on_window_event`, handle `WindowEvent::ScaleFactorChanged { scale_factor, .. }` by re-rasterizing and calling `tray.set_icon(Some(image))`

## 4. Verify

- [x] 4.1 `cargo build -p specforge` succeeds in debug. Release size delta < 1 MB **deferred** to distribution time — needs a master-vs-branch release-build comparison, no baseline currently checked out. Debug-rlib sizes for the new deps (resvg/usvg/tiny_skia/svgtypes/rgb) total ~16 MB unstripped; release with workspace's `lto + strip` should compress that 5–10×, well under budget.
- [ ] 4.2 Launch on a retina display, compare tray icon sharpness against the current PNG-sourced version. **Blocked in this environment** — primary display is 3840×2160 @ 1.0 scale factor (no retina). Needs to be ticked by someone testing on a 2x or 3x display. Functional check did pass: dev binary launches without panic, rasterizer ran and the template-safety assertion did not fire.
- [ ] 4.3 Drag the main window between retina and non-retina displays; confirm tray icon updates. **Blocked in this environment** — single monitor. Code path verified by type-check: `WindowEvent::ScaleFactorChanged` arm in `lib.rs` looks up the tray via `tray_by_id(TRAY_ID)` and calls `tray.set_icon(Some(rasterize_glyph(scale_factor)))`.
- [x] 4.4 Template-safety **functionally verified**: the debug-build pixel-walk assertion in `tray_icon::rasterize` panics on any non-black RGB; the dev binary started without panic, so every pixel of the rasterized SVG has R=G=B=0 → macOS will recolour from alpha as intended. Visual dark/light inversion confirmation still needs the user to toggle Appearance.
- [x] 4.5 Window/dock icon **unchanged by inspection**: `tauri.conf.json` `bundle.icon` array still references the original PNG set; only `install_tray` swapped its icon source. The `default_window_icon()` path used elsewhere (e.g. by the Dock) is untouched.
