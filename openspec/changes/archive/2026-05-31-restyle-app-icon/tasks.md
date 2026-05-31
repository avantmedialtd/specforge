# Tasks

## 1. Prepare source assets

- [x] 1.1 Obtain the new forge artwork as a ≥1024² file — recovered the supplied 1254×1254 PNG from the session image cache; resampled to 1024×1024 opaque and committed as `app-icon.png`
- [x] 1.2 Evaluate the bare-mark squircle candidate's asset need — a frame-free transparent mark would be required; deemed impractical to extract cleanly from the painterly composite, so that candidate was dropped in favour of a framed-squircle variant (no extraction needed)
- [x] 1.3 Produce the 1024×1024 opaque framed master for the framed candidate(s)

## 2. Mock the macOS shapes

- [x] 2.1 ~~Bare-mark `#1a1a1a` squircle candidate~~ — superseded (see 1.2); replaced by the framed-squircle candidate
- [x] 2.2 Build the framed candidates: A (framed square), B (framed squircle, full-bleed ≈22.4% radius), C (framed squircle inset to the 824/1024 margin)
- [x] 2.3 Render all three at 256 px and at 64/32/16 px on a neutral Dock-gray to judge small-size legibility (comparison sheet)

## 3. Decision gate

- [x] 3.1 Compared the three side-by-side; picked **B — framed squircle**, then reversed to **A — framed square** (hard corners, art as supplied) after reviewing it live
- [x] 3.2 Recorded the decision and the deferred `32x32.png` call in `design.md`

## 4. Execute the chosen branch (framed squircle)

- [x] 4.1 Replaced `crates/specforge/icons/app-icon.png` with the 1024×1024 opaque forge master
- [x] 4.2 Regenerated the full derived set via `bun tauri icon … --ios-color "#1a1a1a"`; the macOS `.icns` + bundle PNGs are direct full-bleed **square** rasterizations of the source (no squircle post-process)
- [x] 4.3 Updated the `product-identity` delta (MODIFIED: *Canonical Application Icon Source*, *macOS Icon Tile*) to describe the framed-**square** direction
- [x] 4.4 Confirmed the tray SVGs (`tray-icon.svg`, `tray-specs.svg`) and `src/components/icons.tsx` are untouched (git: unmodified)

## 5. Verify

- [x] 5.1 `bun run build` passes; `cargo build -p specforge` passes (exit 0)
- [x] 5.2 Rebuilt so `generate_context!()` re-bakes the icon; confirmed via macOS Quick Look render of `icon.icns` (the Dock/Finder render path) — full-bleed **square**, opaque corners. Relaunched `bun tauri dev` (binary rebuilt newer than the icns) and captured the live Dock
- [x] 5.3 Spot-checked `icon.icns` (QL renders 512→32), `icon.ico` (`file`: 6-size RGBA), and the iOS 1024 rendition (opaque, min-alpha 255)
- [x] 5.4 Confirmed `bundle.icon` in `tauri.conf.json` still references derivatives (`32x32`, `128x128`, `128x128@2x`, `icon.icns`, `icon.ico`), not the raw `app-icon.png` source

## 6. Small-size legibility (deferred to live review)

- [x] 6.1 Decided from the live render: **accept the full illustration at all sizes** — no simplified `32x32.png` is shipped (menu-bar app; the large Dock/Finder rendition is the primary surface)
