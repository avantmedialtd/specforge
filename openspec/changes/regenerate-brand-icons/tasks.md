# Tasks

## 1. Regenerate desktop + mobile + store icons

- [x] 1.1 Run `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repo root
- [x] 1.2 Confirm the source `crates/specforge/icons/app-icon.png` is unchanged (checksum identical before/after: `443ea493…6cd2`)
- [x] 1.3 Spot-check `icon.icns`: `qlmanage -t -s 256` produced one thumbnail successfully (decodes cleanly)
- [x] 1.4 Spot-check `icon.ico`: `file` reports `MS Windows icon resource - 6 icons, 32x32 with PNG image data, 16x16 with PNG image data` — expected structure
- [x] 1.5 Verified `ios/AppIcon-512@2x.png` (1024×1024 rendition): transparent regions of the source are filled with `#1a1a1a` — dark-anvil mark sits cleanly against the near-black backdrop

## 2. Verify

- [x] 2.1 `bun run build` passes — 509 modules transformed, 512KB JS bundle (pre-existing chunk-size warning unrelated)
- [x] 2.2 `cargo build -p specforge` succeeds in dev profile — confirms nothing in the regenerated icon files breaks compilation
- [ ] 2.3 Visual: dock icon shows the new anvil-and-document mark in the next `bun tauri dev` (or release build). The Tauri dev binary picks up the new `bundle.icon` PNGs on its next compile.
- [ ] 2.4 Stage and commit: `app-icon.png`, all regenerated derivatives, plus this `openspec/changes/regenerate-brand-icons/` directory (the `2026-MM-DD-` date prefix is added later by `openspec archive`)
