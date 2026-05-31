# Restyle the application bundle icon

## Why

A new SpecForge brand illustration has been proposed for the application bundle icon: a painterly forge scene — a rune-marked hammer striking an anvil amid sparks, with a checklist carved into the anvil face. The metaphor is on-brand (forge = build/hammer-out, checklist = OpenSpec changes/tasks) and is a deliberate departure from the current flat mark (a braces document resting on a dark anvil, codified by `regenerate-brand-icons` on 2026-05-25). This change swaps the canonical bundle-icon source to the new art and regenerates the derived icon set.

The new artwork as supplied is a **full-bleed, opaque, stone-framed tile on a near-black background**. That conflicts with the icon contract currently in force in the `product-identity` spec, which mandates (a) a fully **transparent** source containing only the mark and (b) a post-processed **824×824 `#1a1a1a` squircle** tile with a ~10% transparent margin. Resolving that conflict — keep the framed look (amend the spec) or conform to the squircle (reshape the art) — is the central decision of this change, and is deliberately deferred to a side-by-side mock comparison in the real Dock.

Scope is **app-bundle-only**: the menu-bar tray glyphs and the in-app UI icons are explicitly excluded (see Impact).

## What Changes

- Replace the canonical raster source `crates/specforge/icons/app-icon.png` with the new forge artwork, and regenerate every derived bundle icon from it (the `bundle.icon` PNG set, `icon.icns`, `icon.ico`, the iOS Asset Catalog, Android adaptive icons, Windows Store assets) per the frozen recipe in `product-identity`.
- **Decide the macOS tile shape via a mock-compare gate** before committing pixels. Two candidates (detailed in `design.md`):
  - **Squircle (spec-conformant):** extract the forge mark onto a transparent background — dropping the stone frame and dark scene — and let the existing `#1a1a1a` squircle post-process wrap it. No spec change.
  - **Framed tile (art as supplied):** ship the opaque stone-framed square as the tile. Requires modifying two `product-identity` requirements.
  - **Resolved (after a live review):** a **framed square** — the full illustration shipped full-bleed with hard corners, exactly as supplied; the macOS `.icns` + bundle PNGs are direct square rasterizations of the source (no squircle post-process). See `design.md`.
- **Out of scope (explicit):** the menu-bar tray glyphs (`crates/specforge/icons/tray-icon.svg`, `tray-specs.svg`) stay as authored — they are independent black-only templates owned by `tray-indicator`, not derivatives of this source. The in-app UI glyphs (`src/components/icons.tsx`) are untouched.

## Capabilities

### Modified Capabilities

- `product-identity` — **contingent on the shape decision.** If the framed-tile candidate wins, the *Canonical Application Icon Source* requirement (which currently requires a fully transparent source) and the *macOS Icon Tile* requirement (which mandates the transparent-margin `#1a1a1a` squircle composite) are amended to permit a full-bleed opaque framed tile and to drop the squircle post-process. If the squircle candidate wins, `product-identity` is **unchanged** and this becomes a pure asset swap. The delta spec is authored only after the gate, once the branch is known.

### New Capabilities

(none)

## Impact

- **Assets:** `app-icon.png` plus the ~50 regenerated derivatives under `crates/specforge/icons/` are overwritten. The two tray SVGs are explicitly excluded from staging.
- **Spec:** zero or two `product-identity` requirements change, depending on the gate outcome.
- **Code/config:** none expected — `bundle.icon` in `tauri.conf.json` already references the regenerated paths.
- **Asset dependency:** the supplied framed PNG alone cannot drive the squircle candidate — that branch needs a **transparent forge-mark rendition** (frame + dark background removed), which must be sourced or extracted before mocking.
- **Build/verify quirk:** `tauri-build`'s `generate_context!()` bakes icon bytes at compile time, so a full rebuild (invalidate cargo's cache) is required to see the Dock / Cmd-Tab icon actually change — per the precedent recorded in `regenerate-brand-icons`.
- **Risk:** low. No runtime code changes; rollback = revert the commits.
