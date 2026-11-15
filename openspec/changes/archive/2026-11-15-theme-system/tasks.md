## 1. Appearance setting (openspec-app)

- [x] 1.1 In `crates/openspec-app/src/settings.rs`, add `Theme` (`forge` default, `blueprint`, `folio`, `mycelium`) and `ColorScheme` (`system` default, `light`, `dark`), each `#[serde(rename_all = "camelCase")]` with a hand-written `Deserialize` that degrades an unrecognised value to the default, following `DocumentWidth` (`theme-system`: *The Appearance Persists and Reaches Every Window*)
- [x] 1.2 Add `Appearance { theme, scheme }` and an `#[serde(default)] appearance` field to `AppSettings`; add `SettingsStore::appearance()` and `set_appearance()`; re-export the three types from `crates/openspec-app/src/lib.rs`
- [x] 1.3 Add `EVENT_APPEARANCE_CHANGED` (`appearance-changed`, payload the full `Appearance`) to `crates/openspec-app/src/events.rs`
- [x] 1.4 Unit tests in `settings.rs`: defaults are Forge and System; round trip through disk; each wire name; an unrecognised theme and an unrecognised scheme each degrade to the default without failing the rest of the file; a settings file with no `appearance` field loads. Pin the wire shape in `crates/openspec-app/tests/wire_shape.rs`
- [x] 1.5 Run `cargo mutants --in-diff` for the `openspec-app` diff and kill or justify every survivor

## 2. Commands, events and native chrome (specforge, specforge-web)

- [x] 2.1 Add `get_appearance` / `set_appearance` handlers to `crates/specforge/src/commands.rs` (raising `appearance-changed` through the service notice so both transports carry it — see design D14), register both in the `generate_handler!` list in `crates/specforge/src/lib.rs` and in the `APP_COMMANDS` list in `crates/specforge/build.rs`
- [x] 2.2 Allow `get_appearance` in every detached window's command allowlist (`crates/specforge/src/pull_request_window.rs` and the reader and image-window equivalents) so those windows can read the stored value
- [x] 2.3 Add both commands to the `/api/invoke` table in `crates/specforge-web/src/dispatch.rs`, emit `appearance-changed` from the web setter, and carry the event in `crates/specforge-web/src/sse.rs`; add `dispatch::tests` coverage for both arms
- [x] 2.4 In the desktop shell, set each window's native theme from the stored scheme at creation and on `set_appearance` (explicit for Light/Dark, cleared for System), including windows opened later (`theme-system`: *The Scheme Is a Selectable Preference*)
- [x] 2.5 Mirror `Theme`, `ColorScheme`, `Appearance` and the `appearance-changed` event name by hand in `src/types.ts`; add `getAppearance`, `setAppearance` and `onAppearanceChanged` wrappers to `src/api.ts`

## 3. Appearance in the frontend (src/)

- [x] 3.1 Create `src/appearance.ts`: the `localStorage` mirror (read/write, tolerant of absent or unreadable storage), `resolveScheme(scheme, prefersDark)`, and `applyAppearance()` which stamps `data-theme` and the resolved `data-scheme` on `<html>`, sets its `color-scheme` property, and writes the mirror
- [x] 3.2 In `src/main.tsx`, stamp the mirrored appearance before React mounts, beside the `data-doc-width` stamp (`theme-system`: *The Appearance Is In Effect On the First Paint*)
- [x] 3.3 Create `src/hooks/useAppearance.ts` modelled on `useDocumentWidth`: fetch the authoritative value on mount and correct the stamp, listen for `appearance-changed`, re-resolve when the `prefers-color-scheme` media query changes under System, and expose an optimistic `choose()` that restores the previous appearance when the write fails
- [x] 3.4 Replace `useDarkScheme` in `src/components/MermaidBlock.tsx`, `SvgBlock.tsx` and `DashboardView.tsx` with `useAppearance()` so each redraws on a theme change as well as a scheme change, preserving scroll and maximised-figure state; delete `useDarkScheme` once unused (`theme-system`: *Token-Derived Renderings Follow the Active Appearance*)
- [x] 3.5 Replace the two `theme-color` meta tags in `index.html` with one updated at run time from the active `--bg`; set `theme_color` and `background_color` in `public/manifest.webmanifest` to Forge's dark background
- [x] 3.6 `bun test` coverage in `src/appearance.test.ts`: scheme resolution for all three choices, mirror round trip, an unreadable mirror yields Forge and System, and the stamp writes both attributes
- [x] 3.7 **Correction after the browser smoke (corrects 3.2):** the `src/main.tsx` stamp alone is not the first stamp: a module script runs only after the stylesheet has applied, and the cold-start probe caught a rendering opportunity with the root unstamped (the Forge fallback background behind a stored Mycelium in Dark). Stamp first from an inline non-module script in the `<head>` of `index.html`, ahead of the stylesheet link the build appends (design D14), and keep the `main.tsx` stamp; `src/firstPaintStamp.test.ts` holds the copied storage key, ids, default pair and media query equal to `src/appearance.ts` and runs the script against `readMirroredAppearance` and `applyAppearance` across 120 storage and media states. Probe after the fix: no unstamped reading in nine cold starts over three appearances

## 4. Split the stylesheet and extract Mycelium

- [x] 4.1 Capture reference screenshots on `master`, light and dark, of: the tree with a selected and a completed row, a tasks document, a document with a table, code, a diagram and maths, the diff view (unified and side by side), the pull-request view with the review composer, the dashboard, every Settings group, the commit rail, and the empty and error states
- [x] 4.2 Create `src/styles/base.css` and `src/styles/themes/mycelium.css`; move `src/App.css` into them so that `base.css` holds layout, geometry and behaviour and reads tokens only, and `mycelium.css` holds Mycelium's token block (both schemes) and every identity rule, each prefixed `[data-theme="mycelium"]` (`theme-system`: *One Component Tree Serves Every Theme*; `visual-identity`: *Design Token Layer*)
- [x] 4.3 Replace the four `@media (prefers-color-scheme: dark)` blocks with `[data-theme="mycelium"][data-scheme="dark"]` rules; move the syntax-highlight and diff palettes, the workspace swatch text variants and every other scheme-dependent literal into theme tokens
- [x] 4.4 Introduce the tokens that let a differing treatment be a value instead of a rule (inline-code background and ink, selected-row fill and edge, label transform and tracking, task-mark shape) and define them for Mycelium at today's values
- [x] 4.5 Update the stylesheet imports in `src/main.tsx`; keep the divider drag-state rules last in `base.css`; delete `src/App.css`
- [x] 4.6 Temporarily default the stamp to Mycelium and compare against the 4.1 references on every surface in both schemes; fix every difference (`visual-identity`: *Pinned Values Describe the Mycelium Theme*)

## 5. Author Forge and make it the default

- [x] 5.1 Vendor Geist and Geist Mono as Latin-subset variable `woff2` under `src/assets/fonts/` and declare them in `src/fonts.css` with `font-display: swap` and metric-compatible fallbacks
- [x] 5.2 Write `src/styles/themes/forge.css`: the full token vocabulary in light and dark from the design's starting palette, the larger type scale (nothing under 13px), flat elevation tokens, and Forge's structural rules — filled selected row, neutral inline-code chip, sentence-case labels, segmented tabs, square task marks (`theme-system`: *The Theme Is a Selectable Preference*)
- [x] 5.3 Add the bare-`:root` fallback to `base.css`: Forge's tokens with no attribute, and Forge's dark values under `@media (prefers-color-scheme: dark)` guarded by `:root:not([data-scheme])`
- [x] 5.4 Walk every surface from 4.1 in Forge, light and dark, against the canvas mockups; fix anything Forge cannot express by moving the offending declaration from `base.css` into the theme files
- [x] 5.5 Confirm `Theme::default()` is Forge end to end: a fresh config directory opens in Forge on the desktop and in the browser skin

## 6. Author Blueprint and Folio

- [x] 6.1 Vendor IBM Plex Sans, IBM Plex Mono, Instrument Sans and Newsreader as Latin-subset variable `woff2` and declare them in `src/fonts.css`
- [x] 6.2 Write `src/styles/themes/blueprint.css`: tokens in both schemes, zero radii, uppercase mono section labels, the 1px accent outline on the selected row, underlined tabs, hairline-boxed inline code, bracketed mono task marks
- [x] 6.3 Write `src/styles/themes/folio.css`: tokens in both schemes, Newsreader on the reading surface with Instrument Sans chrome, pill tabs, the accent-tinted selected row, plain mono inline code, disc task marks
- [x] 6.4 Walk every surface from 4.1 in Blueprint and in Folio, light and dark; resolve the Folio serif-on-task-lists question recorded in the design's Open Questions and note the outcome in `design.md`
- [x] 6.5 Confirm every theme paints an opaque sidebar under `body[data-platform="mac"]` (the application applies no window vibrancy, so there is no translucency to preserve — see design D10)

## 7. Contrast test

- [x] 7.1 Add `src/themeContrast.ts`: a parser for the flat token blocks of `src/styles/themes/*.css`, per-theme per-scheme resolution, and WCAG relative-luminance and ratio functions
- [x] 7.2 Add `src/themeContrast.test.ts` asserting, for all four themes in both schemes, the pairs required by (`theme-system`: *Every Theme Meets the Same Legibility Floors*): text tones on each surface including hovered and selected rows, primary-button label on its fill, load-bearing edges and the focus indicator, syntax-highlight classes on the code surface and on the diff tints, and that accent, done, error and removal colours differ in lightness
- [x] 7.3 Assert that every theme defines every token of the vocabulary in both schemes (`visual-identity`: *Design Token Layer*, scenario *Every theme defines the whole vocabulary*)
- [x] 7.4 Retune any palette value the test rejects, and remove the hand-measured ratio comments the test now replaces

## 8. Settings: the Appearance group

- [x] 8.1 Add `appearance` to `SETTINGS_GROUPS` and `SETTINGS_GROUP_LABELS` in `src/settingsGroups.ts`, second in the order, and update `src/settingsGroups.test.ts` (`settings-view`: *Settings Are Organised Into Groups*)
- [x] 8.2 Create `src/components/settings/AppearanceGroup.tsx` and wire it into `SettingsView.tsx` and `SettingsNav.tsx`: theme cards in the order Forge, Blueprint, Folio, Mycelium, each previewing inside its own `data-theme` and the resolved `data-scheme`; the System / Light / Dark choice using `ChoiceGroup`; the sample fragment (`settings-view`: *The Appearance Group Offers Theme and Scheme*)
- [x] 8.3 Make the selected card identifiable without colour, keyboard operable, and announced with its name and pressed state; report a failed write beside the control operated and restore the stored appearance (`settings-view`: *Settings Persist by One Rule*)
- [x] 8.4 Component tests: the group order, selecting a card calls `setAppearance` once, a failed write restores the previous selection and shows the error, and the browser host offers the group

## 9. Marketing site

- [x] 9.1 In `site/package.json`, replace `@fontsource/inter` and `@fontsource/jetbrains-mono` with the Geist and Geist Mono packages (run `bun install` with the working directory set to `site/`); update the imports in `site/src/styles.css` and the font allowlist in `site/renderer/+onRenderHtml.tsx`
- [x] 9.2 Replace the token values in `site/src/styles.css` with Forge's, light on `:root` and dark under the existing `@media (prefers-color-scheme: dark)` block; restyle the header, hero, feature blocks, download block and docs prose to Forge; add no theme or scheme control (`marketing-site`: *Site carries its own visual identity*)
- [x] 9.3 Document the Appearance group and the four themes on the relevant `site/pages/docs` page
- [x] 9.4 Retake `site/public/screenshot.png`, `site/public/screenshot.webp` and `docs/screenshot.png` from the app in Forge; leave `site/public/og-specforge.png` untouched
- [x] 9.5 Update `site/e2e` expectations that assert colours, fonts or the screenshot, and run `bun run site:build` and `bun run site:test`

## 11. Mycelium: Meslo and Forge's spacing

- [x] 11.1 Vendor Meslo LG (regular and bold, Latin subset, `woff2`) under `src/assets/fonts/` with its Apache-2.0 licence noted beside the other font notices, and declare it in `src/fonts.css` with `font-display: swap` and a metric-compatible fallback (`visual-identity`: *Typography System*)
- [x] 11.2 In `src/styles/themes/mycelium.css`, set `--font-mono` to Meslo and set every type-size, line-height, document-size and spacing/geometry token to Forge's value; where Forge gets its row and control spacing from `[data-theme="forge"]` rules instead of tokens, lift that spacing into tokens both themes read, without changing Forge's rendering (`visual-identity`: *Pinned Values Describe the Mycelium Theme*, scenario *Mycelium is as spacious as Forge*)
- [x] 11.3 Leave Mycelium's colours, shadows and glows, selection model, chips, label casing and radii untouched; prove it by comparing computed styles against master with Mycelium forced, where only font-family, font-size, line-height, letter-spacing-independent box geometry (padding, margin, gap, width, height, inset) may differ
- [x] 11.4 Extend `src/themeContrast.test.ts` or a sibling test so the type-size and line-height tokens of Mycelium are asserted equal to Forge's, and update the Mycelium theme card blurb in `src/components/settings/AppearanceGroup.tsx` and the site docs page if they describe Mycelium as unchanged
- [x] 11.5 Re-run `bun run lint`, `bun test`, `bun run build` and confirm no text in Mycelium renders under 13px on any surface

## 10. Verification

- [x] 10.1 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test` (after `bun install && bun run build` in this worktree)
- [x] 10.2 `bun run lint`, `bun test`, and `bun run build`; record the bundle size change from the added fonts in `design.md`
- [x] 10.3 Browser smoke with `specforge-serve` + `bun run dev`: walk the scenarios of `theme-system` — fresh config opens in Forge; each theme restyles every surface; System follows an OS change and Light/Dark override it; a second tab adopts a change without reload; a cold start in Mycelium paints no Forge frame; a visible and a maximised diagram redraw in place; an unrecognised stored theme falls back to Forge
- [x] 10.4 Desktop smoke with `bun run wt:dev`: a reader, a pull-request and an image window adopt a theme change while open; detached-window titlebars follow the scheme override; macOS traffic-light clearance holds in all four themes with the sidebar hidden
- [x] 10.5 Full matrix pass — every surface from 4.1 × four themes × two schemes — checking that the rendered element tree is the same across themes for one representative view and that no text in Forge, Blueprint or Folio is under 13px
- [x] 10.6 `openspec validate theme-system --strict`; after archiving, update the "five groups" sentence in the Purpose of `openspec/specs/settings-view/spec.md` and the Purpose of `openspec/specs/visual-identity/spec.md` to describe Mycelium
