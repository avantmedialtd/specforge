## 1. Fix the three call sites

- [x] 1.1 In `crates/specforge/src/tray.rs`, replace the initial-set `tray.set_icon(Some(rasterize_glyph(initial, initial_scale)))` (~line 156) with `tray.set_icon_with_as_template(Some(rasterize_glyph(initial, initial_scale)), true)`
- [x] 1.2 In the same file, replace the variant-flip `tray.set_icon(Some(rasterize_glyph(next, scale)))` (~line 166) with `tray.set_icon_with_as_template(Some(rasterize_glyph(next, scale)), true)`
- [x] 1.3 In `crates/specforge/src/lib.rs`, replace the `ScaleFactorChanged` arm's `tray.set_icon(Some(icon))` (~line 120) with `tray.set_icon_with_as_template(Some(icon), true)`

## 2. Verification

- [x] 2.1 `cargo build -p specforge` compiles
- [x] 2.2 `cargo test -p specforge` passes (no behaviour regression in existing tests)
- [ ] 2.3 Manual: launch with `bun tauri dev` in macOS dark mode, confirm the tray glyph appears in white (not black)
- [ ] 2.4 Manual: trigger a variant flip (create / archive a change directory with a spec delta), confirm the glyph stays in the menu-bar foreground colour after the swap
- [ ] 2.5 Manual: drag the main window between two monitors with different scale factors, confirm the glyph stays in the menu-bar foreground colour after the re-rasterization
