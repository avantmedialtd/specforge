# Regenerate brand icons from refreshed app-icon source

## Why

`crates/specforge/icons/app-icon.png` was replaced on 2026-05-25 with a new SpecForge brand asset — a dark anvil bearing a small spec document with curly-brace content and an orange page-curl. Every derived bundle icon (`bundle.icon` PNG set, `icon.icns`, `icon.ico`, the iOS Asset Catalog, Android adaptive icons, Windows Store assets) still reflects the previous artwork, dated 2026-05-24. The shipping bundle therefore carries mismatched branding: dock icon and installer surfaces still show the old logo.

## What Changes

- Regenerate the full derived icon set from `app-icon.png` via `bun tauri icon`, with `--ios-color "#1a1a1a"` so iOS renders the dark-anvil mark against a near-black backdrop rather than the default white (which clashes with the dark anvil silhouette).
- Add a macOS tile post-processing step: composite the transparent source onto an 824×824 `#1a1a1a` squircle (185 px radius, centered in a 1024×1024 canvas), pack a fresh `icon.icns` via `iconutil`, and re-render the bundle window-icon PNGs (`32x32`, `64x64`, `128x128`, `128x128@2x`, `icon.png`) from the same tiled master so the dock icon in `bun tauri dev` and the Linux launcher icon match. macOS Finder/Dock requires an opaque rounded-rect tile — the bare anvil silhouette on transparency reads wrong on the dock and grey desktop backgrounds. The same `#1a1a1a` color used for iOS keeps the platforms visually consistent.
- Preserve `app-icon.png` as the canonical 1024×1024 source — fully transparent — so iOS/Android/Windows derivatives can compose against their own per-platform backgrounds.
- `app-icon.png` remains the only authored raster source for the bundle icons; everything else in `crates/specforge/icons/` is mechanically regenerated. The two tray glyph SVGs (`tray-icon.svg`, `tray-specs.svg`) are explicitly out of scope — they're independently authored vector templates managed under the `tray-indicator` capability.

## Capabilities

### Modified Capabilities

- `product-identity`: gains a new requirement codifying `crates/specforge/icons/app-icon.png` as the canonical 1024×1024 raster source for every bundle icon, with the regeneration command (`bun tauri icon … --ios-color "#1a1a1a"`) and the iOS composite color frozen as part of the contract. This formalises a source-of-truth claim that was previously tribal knowledge — nothing in the existing specs said where bundle icons came from.

### New Capabilities

(none)

## Impact

- **Assets**: every file under `crates/specforge/icons/` *except* `app-icon.png`, `tray-icon.svg`, and `tray-specs.svg` gets overwritten by `tauri icon`. `icon.icns` and the bundle PNGs (`32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`) are then overwritten a second time with the tiled versions produced by the ImageMagick + `iconutil` recipe documented in tasks.md §2 and codified in the `macOS Icon Tile` requirement. The Linux `.deb`/`.appimage` shipped icons therefore also carry the tile — accepted per the cross-platform PNG sharing in `bundle.icon`. No `.rs`, `.ts`, `.css`, or config files change.
- **Build**: `bundle.icon` in `tauri.conf.json` already references the regenerated paths — no config update needed.
- **Diff size**: ~50 binary files change (plus the larger `.icns` — now ~340 KB to hold the 10 macOS sizes through 1024@2x, vs ~28 KB previously).
- **Risk**: low. No code changes; the spec deltas codify behaviour rather than introducing it. Rollback = revert the commits.
