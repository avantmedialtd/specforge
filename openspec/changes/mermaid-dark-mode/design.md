# Mermaid Dark-Mode Theming — Design

## Context

`MermaidBlock.tsx` renders ```mermaid fences with mermaid 11.16.0, using `theme: "base"` plus a `themeVariables()` map that reads design tokens live off `:root` (`mainBkg: --surface-2`, `textColor: --text`, …). Mermaid's base theme treats supplied variables as-is, but *derives* every unsupplied variable from the supplied ones — and the direction of derivation is steered by a `darkMode` boolean that the map never sets. With `darkMode` falsy, the engine assumes the palette is light and derives by lightening: for ER diagrams, `rowOdd = lighten(mainBkg, 75)` (≈ white against `--surface-2: #1b212b`) while `rowEven = lighten(mainBkg, 5)` stays dark. Attribute text is the supplied `--text` (near-white), so odd rows are white-on-white.

Two engine details matter:

- The v11 ER renderer (`rendering-elements/shapes/erBox.ts`) fills rows from the theme variables `rowOdd` / `rowEven`. The *documented* ER variables `attributeBackgroundColorOdd/Even` are dead — nothing in the v11 code path reads them.
- The component already tracks the active scheme via `useDarkScheme()`, but only to re-key the render effect; the value never reaches the theme map.

## Goals / Non-Goals

**Goals:**

- ER attribute rows (and any other engine-drawn surface) legible under `--text` in both light and dark schemes.
- Every variable mermaid derives — across all diagram types, not just ER — derived in the direction of the active scheme.
- Keep the file's invariant: no literal colours; everything reads from tokens or the tracked scheme.

**Non-Goals:**

- Relationship-label overlap in dense ER graphs (upstream dagre layout; no theming hook exists).
- Any change to which fences render as diagrams, error fallback, or the strict security posture.
- Restyling diagrams that are already legible; this corrects derivation, not the design language.

## Decisions

### D1: Pass `darkMode` into the base theme, keyed off `useDarkScheme()`

`themeVariables()` gains the scheme flag (threaded from the component's existing `isDark` — one source of truth, no second `matchMedia` read) and the map sets `darkMode: isDark`. This flips every derivation in the engine to the correct direction at once — borders via `mkBorder`, edge-label fallbacks, pie/gantt/git palettes — not just the ER rows that made the bug visible.

*Alternative — switch to `theme: "dark"` when dark:* rejected. The dark theme ships mermaid's stock dark palette; the token map would still override most of it, stock colours would leak through the gaps we don't map, and the code would fork into two theme paths for no gain over one dark-aware base theme.

*Alternative — explicitly supply every variable mermaid would otherwise derive:* rejected. The base theme derives dozens of variables across diagram types and grows more with each release; enumerating them is unmaintainable and each miss reproduces this bug. `darkMode` fixes the class, not the instance.

### D2: Pin `rowOdd` / `rowEven` to tokens rather than trusting corrected derivation

Even with `darkMode: true`, row fills would come from the engine's `darken(mainBkg, 5|10)` — a computed colour, not a token, with contrast at the engine's discretion. Instead the map sets them explicitly: `rowOdd: --surface-2` (rows sit flush with the entity box) and `rowEven: --surface-3` (one surface step up), the same one-step alternation the app's own UI uses, in both schemes, with `--text` legibility guaranteed by the token system itself.

*Alternative — set the documented `attributeBackgroundColorOdd/Even`:* rejected — dead variables in the v11 renderer (see Context); setting them changes nothing.

*Alternative — rely on D1's corrected derivation alone:* rejected as the primary mechanism — it works, but it hands row contrast to engine colour math and violates the tokens-only principle. D1 still matters as the safety net for everything the map doesn't pin (and would keep rows sane if a future mermaid renames the row variables again).

## Risks / Trade-offs

- [`darkMode: true` shifts *many* derived colours at once — other diagram types could change appearance in dark scheme] → That is the point (they were mis-derived too), but verify visually: smoke a gallery of diagram types (flowchart, sequence, state, ER) in both schemes via the `specforge-web` debug build before shipping.
- [`rowOdd`/`rowEven` are undocumented v11 internals and could be renamed upstream] → D1 keeps derivation dark-aware as a fallback, so a rename degrades to "engine-computed but legible" rather than white-on-white; a comment at the mapping records why these names, so a future bump re-checks them.
- [Light scheme changes subtly: rows that previously derived near-white now use `--surface-2`/`--surface-3`] → Intended alignment with the app's surface ladder; covered by the same two-scheme visual smoke.
