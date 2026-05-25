# Restyle: Engineered Minimal

## Why

SpecForge's current chrome is competent but anonymous — system font stack, ad-hoc spacing and type sizes, semi-transparent gray helpers, and the raw macOS system blue as the only accent. It reads as "stock Apple tool" rather than a product with a point of view. We want a stylish, recognizable visual identity that costs us very little to maintain: no UI framework, no Tailwind, just a small token system, two web-vendored typefaces, and a single accent. The app already does both glance-driven triage (tray badge, tree scanning) and deep reading (detail pane), so the new style needs to hold up under both modes.

## What Changes

- **New design tokens** (CSS custom properties) for color, type, space, radii, and borders. Replace the current 5 ad-hoc variables and inline magic numbers with a real scale.
- **Typography system**: vendor Inter Variable and JetBrains Mono Variable locally. Mono is used as a *type system* — change IDs, branch names, timestamps, progress, file paths — not only for code blocks.
- **Single accent**: Linear indigo (`#5e6ad2`) used for selection bar, focus ring, primary button, and links. Replaces the raw `rgb(0, 122, 255)` macOS system blue.
- **Cool neutral palette**: explicit light/dark scales with a slight blue tint. Replaces `rgba(127, 127, 127, …)` helpers.
- **Selection model**: tree-row selection becomes a 2px accent left bar with a 6% accent-tinted background, instead of the current 18% blue fill. Hover is a 3-4% neutral shift.
- **Badges become outlined chips**: `[ MISSING ]`, `[ DIVERGED ]`, etc. render in mono caps with a 1px outline and no fill. Divergence collapses to a 4px status dot where space is tight.
- **Uniform row grammar**: tree rows, settings workspace rows, and any future list surfaces share one row template (height, padding, type sizes, divider treatment).
- **macOS niceties**: sidebar vibrancy via `window-vibrancy`; titlebar hidden with inset traffic lights floating over the sidebar. Windows/Linux render the same chrome on solid surfaces.
- **Inline SVG icons** replace the placeholder glyphs `▸ ▾ ● ✕` with a consistent set (5 icons, hand-rolled in a single `icons.tsx` to stay dependency-free).
- **Markdown typography only**: detail-pane prose adopts the new type scale and Inter / mono, but a deeper markdown overhaul (callouts, code-block chrome, anchor links) is explicitly out of scope and left as a follow-up.

## Capabilities

### New Capabilities

- `visual-identity`: the app-wide design contract — tokens, typography, color, spacing, selection model, badges, icons, and macOS-specific window chrome (sidebar vibrancy, hidden inset titlebar).

### Modified Capabilities

<!-- None. spec-browser, tray-indicator, and workspace-registry retain their existing behavioural requirements. Their visual presentation is governed by the new visual-identity capability rather than re-stated in their own specs. -->

## Impact

- **`src/App.css`**: refactor from ~640 ad-hoc lines into a token-led stylesheet of similar size. Token definitions live in `:root` and `:root[data-theme="dark"]` (or `@media (prefers-color-scheme: dark)`).
- **`src/assets/fonts/`** (new): vendor Inter Variable + JetBrains Mono Variable as `woff2`. ~80-120KB on disk, zero network.
- **`src/components/icons.tsx`** (new): inline SVG icons for chevron, settings, close, present/absent state.
- **`src/components/*.tsx`**: update class names and inline glyphs to use the new tokens and icon components. No behavior changes.
- **Tauri shell (`crates/specforge/`)**: add `window-vibrancy` crate; enable sidebar vibrancy on macOS; switch window to hidden-inset titlebar on macOS. Other platforms unaffected.
- **`index.html`**: declare `@font-face` for the vendored fonts; ensure no flash by using `font-display: block` for the small first-paint window or `swap` with a metric-compatible fallback.
- **No changes** to `openspec-core`, IPC types, settings persistence, watcher behavior, or notification logic.
- **Bundle size**: net increase ~80-120KB for fonts. No JS framework added.
- **Cross-platform**: Mac gets the full treatment; Windows/Linux get the same color/type/selection/badge system on solid (non-vibrant) surfaces with a stock titlebar.
