# Add the Avant theme

## Why

SpecForge is an Avant Media product, but none of its four themes looks like Avant Media. The studio's sites have settled on a flat, dark editorial style — black ground, white ink, hot pink for actions, square edges — and a reader who arrives from them finds no trace of it in the app.

## What Changes

- Add a fifth selectable theme, **Avant**, after Mycelium. Forge stays the default and the SpecForge brand.
- Avant takes the brand's ink, edges and one loud colour: untinted black-and-white neutrals, hot pink (`#ff3366`) kept for what can be acted on, neon green (`#00ff88`) kept for done, square corners, hard offset shadows with no blur, uppercase mono section labels, and Archivo Black for the two top document heading levels.
- Avant keeps what makes SpecForge a reading tool: Forge's type sizes, line heights and spacing, nothing under 13px, Inter for prose, every legibility floor met with no exception, and none of the brand's rotation, skew, stacked text shadows or gradients.
- Avant supplies a light scheme as every theme does: the same page printed, white paper and black ink.
- Vendor one font file, Archivo Black (SIL OFL 1.1, about 19 KB). Inter and JetBrains Mono are already shipped.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `theme-system`: five themes instead of four; Avant's identity joins the table; Avant is held to every legibility floor with no exception.
- `settings-view`: the Appearance group offers a fifth card, Avant, last in the order.

## Non-goals

- Changing the default theme, or the marketing site's fixed use of Forge.
- The brand's motion and its transforms (rotated headings, skewed buttons, card underlays).
- A display-face token in the shared vocabulary. Archivo Black is named in Avant's own rules only.
- The terminal frontend, which has no themes.

## Impact

- **Frontend**: `src/styles/themes/avant.css` (new), `src/fonts.css`, `src/assets/fonts/ArchivoBlack-400.woff2` (new), `src/main.tsx`, `src/appearance.ts`, `src/types.ts`, `index.html` (the first-paint stamp's hand copy), `src/components/settings/AppearanceGroup.tsx`.
- **Service**: `Theme::Avant` in `crates/openspec-app/src/settings.rs`, wire name `avant`. A settings file written by this version and read by an older one folds `avant` to Forge, as any unknown theme does.
- **Tests**: `themeContrast`, `appearance`, `firstPaintStamp`, `AppearanceGroup`, `wire_shape.rs`, the settings round trip, and `scripts/check-pr-content-loading.js`'s theme loop.
- **Docs**: the marketing site's Settings page lists five themes; `src/CLAUDE.md`.
