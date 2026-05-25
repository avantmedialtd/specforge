# Tasks

## 1. Regenerate desktop + mobile + store icons

- [x] 1.1 Run `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repo root
- [x] 1.2 Confirm the source `crates/specforge/icons/app-icon.png` is unchanged (checksum identical before/after: `443ea493…6cd2`)
- [x] 1.3 Spot-check `icon.icns`: `qlmanage -t -s 256` produced one thumbnail successfully (decodes cleanly)
- [x] 1.4 Spot-check `icon.ico`: `file` reports `MS Windows icon resource - 6 icons, 32x32 with PNG image data, 16x16 with PNG image data` — expected structure
- [x] 1.5 Verified `ios/AppIcon-512@2x.png` (1024×1024 rendition): transparent regions of the source are filled with `#1a1a1a` — dark-anvil mark sits cleanly against the near-black backdrop

## 2. Add the macOS icon tile

`tauri icon` produces a transparent-source `.icns` whose Finder/Dock rendering would be the bare anvil silhouette on no tile. macOS app icons need an opaque rounded-rect tile. The source `app-icon.png` stays transparent (so iOS/Android/Windows composites still work); the tile is added in a post-processing step over the `tauri icon` output.

- [x] 2.1 Composite a tiled 1024×1024 PNG: 824×824 squircle (185 px radius, ≈22.4% per macOS Big Sur+ convention) centered, filled `#1a1a1a`, with the source scaled to 820×820 and composited over the tile. Single-chain ImageMagick to avoid the colorspace-flattening that occurs in multi-step pipes: `magick \( -size 1024x1024 xc:none -fill "#1a1a1a" -draw "roundrectangle 100,100 923,923 185,185" \) \( app-icon.png -resize 820x820 -background none -gravity center -extent 1024x1024 \) -compose Over -composite -colorspace sRGB PNG32:tiled.png`
- [x] 2.2 Generate an iconset folder with the ten canonical macOS sizes (16/32/128/256/512 at @1x and @2x), then pack with `iconutil -c icns AppIcon.iconset -o crates/specforge/icons/icon.icns`
- [x] 2.3 Verified via `qlmanage -t -s 512`: tile renders as black squircle with transparent margin, anvil silhouette preserved with subtle highlights, document with orange braces and page-curl visible and correctly colored
- [x] 2.4 Overwrite the bundle PNG set from the same tiled master so the window/dock icon during `bun tauri dev` (and the Linux launcher icon) matches the `.icns`: `for pair in 32:32x32 64:64x64 128:128x128 256:128x128@2x 512:icon; do magick tile.png -resize $size -colorspace sRGB PNG32:icons/${name}.png; done` — overwrites `32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`
- [x] 2.5 Verified the tiled PNGs at 32×32 and 256×256: tile geometry preserved at small size; color channels unequal (R≠G≠B confirms the orange + dark-gradient are intact)

## 3. Verify

- [x] 3.1 `bun run build` passes — 509 modules transformed, 512KB JS bundle (pre-existing chunk-size warning unrelated)
- [x] 3.2 `cargo build -p specforge` succeeds in dev profile — confirms nothing in the regenerated icon files breaks compilation
- [x] 3.3 Verified end-to-end: rebuilt after `touch tauri.conf.json` to invalidate cargo's cache (tauri-build's `generate_context!()` bakes icon bytes at compile time — restarting `bun tauri dev` alone reuses the stale binary). Extracted the largest embedded PNG from `target/debug/specforge` (offset `0xe0d90e`, 1024×1024, 191 KB) — it is the tiled squircle master with the SpecForge mark inside. User confirmed dock + Cmd+Tab show the tiled icon after `killall Dock`.
- [x] 3.4 Staged 55 files (`app-icon.png` + 51 regenerated derivatives + 4 OpenSpec artifacts under `openspec/changes/regenerate-brand-icons/`) and committed as `c7701f2`. Tray SVGs explicitly excluded from staging. Not pushed to upstream.
- [x] 3.5 Committed as `2ea966b` (9 files: 6 tiled assets + 3 OpenSpec artifacts) on top of `c7701f2`. Tray SVGs verified absent from the staged set. Not pushed to upstream.
