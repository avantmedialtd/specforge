## 1. CSS row grammar

- [x] 1.1 Remove `border-radius: var(--radius)` from `.tree-row` in `src/App.css` (keep the 2px transparent left border so the selection bar still slides in without shifting label content)
- [x] 1.2 Remove `margin: 0 var(--space-1)` from `.tree-row` in `src/App.css`
- [x] 1.3 Confirm hover rules (`.tree-row:hover` and `.tree-row--tinted:hover`) still resolve to the same backgrounds — only the row geometry has changed, not the hover composition

## 2. CSS selection rules

- [x] 2.1 Remove `background: var(--accent-tint)` from `.tree-row.selected` in `src/App.css` (keep `border-left-color: var(--accent)`)
- [x] 2.2 Remove the entire `.tree-row--tinted.selected` rule in `src/App.css` (the linear-gradient composition is no longer needed — the workspace tint is already painted by `.tree-row--tinted`, and the 2px accent left bar from `.tree-row.selected` composes on top by itself)
- [x] 2.3 Grep the stylesheet for other `--accent-tint` callers and confirm the token still has at least one consumer outside the tree-row rules; if not, note in the implementation summary so the token's continued definition can be revisited separately

## 3. Spec sync

- [x] 3.1 Apply the visual-identity delta from `openspec/changes/flatten-sidebar-rows/specs/visual-identity/spec.md` to `openspec/specs/visual-identity/spec.md` (modify the Accent Color and Tree Row Selection Model requirements, add the Flat Tree Row Geometry requirement) — this step runs at archive time via `openspec archive`

## 4. Manual verification

- [x] 4.1 Run `bun tauri dev` and confirm: top-level tinted rows render edge-to-edge with no rounded corner and no side gutter
- [x] 4.2 Click a tinted top-level row — confirm the 2px accent left bar appears and the workspace tint remains visible underneath (no extra background change)
- [x] 4.3 Click an untinted child row (e.g. `Proposal`) — confirm only the accent left bar appears, no fill
- [x] 4.4 Hover an unselected tinted row — confirm the existing multiply-blend hover wash still composes correctly over the tint
- [x] 4.5 Hover an unselected untinted row — confirm the `--surface-2` hover background still appears
- [x] 4.6 Tab to a row with the keyboard — confirm the focus outline (`outline: 2px solid var(--accent)`) still renders inside the row
- [x] 4.7 Stack three workspaces with no expansions and observe whether adjacent tint bands read as a unified list or as jarring stripes — note the outcome in the implementation summary; do not add a divider or alpha change as part of this task (revisit only if explicitly required by the result)

## 5. Build check

- [x] 5.1 Run `bun run build` and confirm `tsc --noEmit` plus the Vite build succeed (no TypeScript or CSS errors introduced by the change)
- [x] 5.2 Run `cargo test` and confirm no Rust tests are affected (none should be — this is a pure CSS change, but the run confirms no accidental Rust edits)
