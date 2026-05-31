# Design — Restyle the application bundle icon

## Context

The bundle icon is a **governed surface**. The `product-identity` spec (established by `regenerate-brand-icons`, 2026-05-25) freezes three things:

1. **Canonical source** — `crates/specforge/icons/app-icon.png`, a 1024×1024 **RGBA, fully transparent** PNG holding only the mark.
2. **Regen recipe** — `bun tauri icon crates/specforge/icons/app-icon.png --ios-color "#1a1a1a"` from the repo root, which derives the entire desktop/mobile/store icon set.
3. **macOS tile post-process** — `tauri icon` alone produces a transparent-source rasterization that reads wrong on the Dock, so the `.icns` and bundle PNGs are overwritten with a composite: the transparent mark scaled to 820×820 over an **824×824 `#1a1a1a` squircle** (185 px radius ≈ 22.4%) centered in a 1024×1024 canvas with a ~10% transparent margin. Built via a single-chain ImageMagick command + `iconutil`.

The repo deliberately chose this squircle-on-`#1a1a1a` look because macOS does **not** mask app icons (unlike iOS) — a bare silhouette on transparency, or a hard full-bleed square, both read wrong next to native squircle icons.

## The decision: framed tile vs squircle

The new artwork as supplied is the opposite of what the contract assumes: an **opaque, full-bleed, stone-framed square** on a near-black background, with internal detail (an "S" rune on the hammer, sparks, a 3-row checklist carved into the anvil).

| | **Squircle** (conform) | **Framed tile** (art as sent) |
|---|---|---|
| Spec impact | none — pure asset swap | modifies 2 `product-identity` reqs |
| Source asset | transparent forge **mark** only (frame + bg stripped) | the opaque framed PNG as-is |
| Fidelity to the art | low — loses the stone frame + dark scene, i.e. most of its character | high — ships exactly what was designed |
| macOS native fit | high — matches the ratified squircle house style | lower — hard square among native squircles |
| Pipeline | existing recipe + squircle composite, unchanged | bypass/replace the squircle post-process |

The honest tension: **the squircle path doesn't "reshape" the art, it guts it.** The frame and dark scene are what make the illustration distinctive; extract just the anvil+hammer onto transparency and the result is a different, plainer mark. So the comparison is less "two shapes of the same icon" and more "the full illustration in a hard frame" vs "a stripped mark in the house squircle." That is exactly why a visual gate beats arguing it on paper.

### If the framed tile wins — the spec amendment

Two `product-identity` requirements would change (delta authored post-gate):

- **Canonical Application Icon Source** — its *"Source remains transparent"* scenario forbids opaque pixels outside the mark; a framed tile is opaque edge-to-edge. The requirement would be relaxed to allow an opaque, pre-composed source (and the `--ios-color "#1a1a1a"` clause becomes moot for the composite, though it stays harmless).
- **macOS Icon Tile** — the entire squircle-composite contract (transparent margin, 824/1024 area, 185 px radius, `#1a1a1a` fill) is contradicted by a full-bleed framed square. Either drop the requirement or rewrite it to describe the framed tile's own geometry (and decide whether to round the outer corners to soften the square against the Dock).

If the squircle wins, neither requirement changes.

## Asset requirements

- A **≥1024² master** of the new art (confirm exact dimensions/format on receipt; the contract wants 1024×1024).
- For the squircle candidate: a **transparent forge-mark** rendition — frame and dark background removed — that the `#1a1a1a` squircle can sit behind. This does not exist yet and must be produced (manual extraction or a re-export from the original art source).
- For the framed candidate: a clean **1024×1024 opaque** rendition of the framed tile.

## Small-size legibility

The bundle icon must read at 32×32 (Finder list, notifications) and the mark is glanced at 16×16 elsewhere. The framed art is busy — stone frame, rune, sparks, and a 3-row checklist all compete and turn to mud below ~64 px. Tauri lets us ship a **distinct `32x32.png`** (a simplified small-size rendition) if the full art doesn't survive shrinking. Evaluate this during the mock gate; note it as a follow-up task if needed rather than blocking the decision.

## Notes carried from exploration

- **The "S" rune** reads as a generic glyph and could be mistaken for another brand at a glance. `product-identity` is strict about the SpecForge-vs-OpenSpec distinction; flag the rune as an intentional choice, not the wordmark.
- **Tray stays put.** The menu-bar glyph is a separate black-only template (`tray-indicator`); none of this art can drive it (a 16 px pure-black silhouette can't carry the rune/sparks/checklist). Pulling the tray into scope was explicitly declined.

## Decisions (resolved at the gate)

1. **Source asset** — the supplied 1254×1254 forge illustration (recovered from the session image cache) is resampled to a 1024×1024 opaque square and committed as `app-icon.png`. No frame-free transparent rendition was needed; the bare-mark squircle candidate was dropped (see below).
2. **Shape — framed square (candidate A).** Initially shipped as the rounded squircle (B), then reversed to the hard-cornered full-bleed square after reviewing it live — the forge illustration exactly as supplied, edge-to-edge. The macOS `.icns` and bundle PNGs are direct rasterizations of the opaque square source (no squircle/tile post-process). The inset squircle (C) and the spec's original bare-mark-on-`#1a1a1a` squircle were not pursued.
3. **Small sizes — accept the full illustration.** Below ~48 px it muddies to a dark tile with a yellow anvil glow; judged acceptable from the live macOS render. No simplified `32x32.png` is shipped — this is a menu-bar app and the large Dock/Finder rendition is the primary surface.

## Implementation note

There is no macOS post-process: the canonical `app-icon.png` is a full-bleed opaque square, so `icon.icns` (packed via `iconutil`) and the bundle PNGs are direct square rasterizations of it — the same full-bleed art every platform derives from. This departs from the `regenerate-brand-icons` `#1a1a1a` squircle-tile architecture, which assumed a transparent mark; the framed illustration carries its own background and frame.
