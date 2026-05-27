## 1. CSS token cleanup

- [x] 1.1 Delete the eight `--ws-tint-<colour>` declarations from the light-scheme `:root` block in `src/App.css`
- [x] 1.2 Delete the eight `--ws-tint-<colour>` declarations from the dark-scheme `@media (prefers-color-scheme: dark) :root` override block in `src/App.css`
- [x] 1.3 Confirm no other rule in `src/App.css` references `--ws-tint-*` after deletion (grep `--ws-tint`)
- [x] 1.4 Update the palette-tokens comment block above the `:root` declarations so it describes only the swatch tokens (the tint tokens are gone) — keep the comment accurate, not aspirational

## 2. CSS row-tint rule cleanup

- [x] 2.1 Delete the `.tree-row--tinted` rule from `src/App.css` (the rule that sets `background: var(--tree-row-tint, transparent)`)
- [x] 2.2 Delete the eight `.tree-row--tinted.tree-row--tint-<colour>` rules that resolve `--tree-row-tint` per palette colour
- [x] 2.3 Delete the `.tree-row--tinted:hover` rule (the linear-gradient + `background-blend-mode: multiply` hover composition)
- [x] 2.4 Confirm the remaining `.tree-row:hover { background: var(--surface-2); }` rule applies uniformly to every row (no per-depth or per-tint exception remains)

## 3. CSS swatch + divider

- [x] 3.1 Add a `.row-swatch` class to `src/App.css` that renders an 8×8 filled circle (`width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;`)
- [x] 3.2 Add eight `.row-swatch.row-swatch--<colour>` per-palette rules that set `background: var(--ws-swatch-<colour>)`
- [x] 3.3 Add the inline-layout rule that sits the swatch between the chevron and the label with a `gap` matching the existing chevron→content spacing (verify against `.row-icon` for parity)
- [x] 3.4 Add a top-divider rule that resolves `border-top: var(--border-width) solid var(--border)` on every top-level tree row except the first — implement via either a structural selector (e.g. `.tree > div:not(:first-child) > .tree-row:first-child`) or a `tree-row--top-level` modifier class with `:not(:first-of-type)` — pick whichever the surrounding CSS makes most readable

## 4. TSX — `Row` primitive

- [x] 4.1 In `src/components/WorkspaceTree.tsx`, add a `swatch?: PaletteColor | null` field to the `RowProps` interface; remove the `tint?: PaletteColor | null` field
- [x] 4.2 Remove the `tintClass` derivation from the `Row` function body
- [x] 4.3 Add a swatch-class derivation: when `swatch` is non-null, compose `row-swatch row-swatch--<swatch>` for the swatch element's className
- [x] 4.4 Render the swatch element in `Row` between the chevron span and the row-label span — a `<span className={swatchClass} aria-hidden="true" />` (no SVG needed for a flat-coloured circle; CSS background-colour on a square `<span>` with `border-radius: 50%` is sufficient)
- [x] 4.5 If using the modifier-class approach for the divider, add `tree-row--top-level` to the root row className when the row is at depth 0 (controlled by a new `isTopLevel?: boolean` prop or inferred from `depth === 0`)
- [x] 4.6 Confirm the `Row` JSDoc comment is updated — the old comment on `tint` ("Renders as a dim background on the row only…") is replaced by a `swatch` comment ("Identity glyph rendered between chevron and label, top-level rows only")

## 5. TSX — call sites

- [x] 5.1 In `RepoNode` (`src/components/WorkspaceTree.tsx`), replace `tint={repo.color}` with `swatch={repo.color}` on its `Row` invocation; add `isTopLevel` (or equivalent) if the divider uses the modifier-class approach
- [x] 5.2 In `FlatWorkspaceNode` (`src/components/WorkspaceTree.tsx`), replace `tint={color}` with `swatch={color}` on its `Row` invocation; add `isTopLevel` (or equivalent)
- [x] 5.3 Confirm no other call site of `Row` in `WorkspaceTree.tsx` passes a `tint` prop (the prop is removed; any leftover usage is a TypeScript error)

## 6. Spec sync (archive-time)

- [x] 6.1 Apply the spec-browser delta from `openspec/changes/swap-row-tint-for-swatch/specs/spec-browser/spec.md` to `openspec/specs/spec-browser/spec.md` (remove `Top-Level Row Display Name and Tint`; add `Top-Level Row Display Name and Swatch` and `Inter-Workspace Divider`) — runs automatically via `openspec archive`
- [x] 6.2 Apply the visual-identity delta from `openspec/changes/swap-row-tint-for-swatch/specs/visual-identity/spec.md` to `openspec/specs/visual-identity/spec.md` (modify `Accent Color` and `Tree Row Selection Model` requirements) — runs automatically via `openspec archive`

## 7. Manual verification

- [x] 7.1 Run `bun tauri dev`; verify a workspace with a configured palette colour renders an 8px filled dot between the chevron and the label, and the row background is the default surface (no tint band)
- [x] 7.2 Verify a workspace with no configured palette colour renders no swatch and no leading gap (label sits immediately after the chevron)
- [x] 7.3 Verify two or more top-level rows are separated by a 1px `--border` hairline; the first top-level row carries no top border
- [x] 7.4 Click a top-level row that has a palette colour — confirm the 2px `--accent` left border appears AND the swatch remains visible AND the row background is unchanged
- [x] 7.5 Hover an unselected top-level row — confirm the background flips to `--surface-2` uniformly (no multiply-blend composition over a tint)
- [x] 7.6 Hover an unselected child row (e.g. `Proposal`) — confirm `--surface-2` hover background applies identically to the top-level case
- [x] 7.7 Tab to a row with the keyboard — confirm the focus outline (`outline: 2px solid var(--accent)`) still renders inside the row, composes correctly with the swatch and divider
- [x] 7.8 Open Settings, change a workspace's palette colour — confirm the tree-pane swatch updates without a manual refresh (the existing presentation-store reactivity is preserved)
- [x] 7.9 Open Settings, clear a workspace's palette colour (set to "none") — confirm the swatch disappears from the tree-pane row

## 8. Build check

- [x] 8.1 Run `bun run build` and confirm `tsc --noEmit` plus the Vite build succeed (the removed `tint` prop on `Row` should cause a TypeScript error if any call site still passes it — that's the expected failure mode if step 5.x missed a call site)
- [x] 8.2 Run `cargo test` and confirm no Rust tests regress (this is a pure UI change; the run confirms no accidental Rust edits)
