## 1. Tree foundations (memoization + roving tabIndex)

- [x] 1.1 Make `toggle` a stable `useCallback` in `WorkspaceTree`, distribute selection through an external store (`useSyncExternalStore` per Row) instead of the threaded `selectedNodeId` string, and wrap the top-level node components in `React.memo` (D3) — a selection change re-renders only the rows whose selected bit flipped
- [x] 1.2 Render every `Row` with `role="treeitem"` and `tabIndex={-1}`; a post-render effect imperatively promotes exactly one row (the focused-id ref, falling back to the first visible row) to `tabIndex={0}` (D2)

## 2. ARIA semantics

- [x] 2.1 Add `role="tree"` (with an `aria-label`) to the tree container, `role="group"` to every children wrapper, and explicit `aria-level` on every row (pass depth through the existing `depth` prop)
- [x] 2.2 Add `aria-expanded` on expandable rows, `aria-selected` on the selected row, and `aria-disabled="true"` + focusable-but-inert behavior on dim missing-artifact rows

## 3. Key handling

- [x] 3.1 Add the single container `keydown` handler: ArrowDown/ArrowUp move focus through `querySelectorAll('[role="treeitem"]')` order (comment the DOM-order invariant per D1 risk), Home/End jump to extremes, with `scrollIntoView({block:"nearest"})` on each move
- [x] 3.2 ArrowRight: expand collapsed row via the chevron's own click handler / move to first child when expanded; ArrowLeft: collapse expanded row / jump to parent (nearest preceding row with a smaller `aria-level`)
- [x] 3.3 Enter/Space: content rows (instance/proposal/design/tasks artifact/capability-spec/section/task) fire the row's own click exactly as a pointer would; grouping rows (incl. the Specs artifact row) toggle expansion; `aria-disabled` rows are inert
- [x] 3.4 First-letter typeahead: single-character, case-insensitive, wrapping search of visible rows' primary-label text after the current row (D6)
- [x] 3.5 Debounced follow-focus (D4): 150 ms settle timer on focus move clicks content rows only, guarded by activeElement + already-selected re-checks at expiry; Enter/Space cancels the timer; grouping and disabled rows never start it
- [x] 3.6 Focus fallback (D5): capture the focused row's ancestor-id chain at focus time (scanning back through decreasing `aria-level`s); when a views refresh removes the row, the post-render effect walks the chain to the nearest surviving ancestor and restores focus if it was lost to the body; fall back to the first visible row

## 4. Shell periphery

- [x] 4.1 `SplitPane` dividers: `tabIndex={0}`, ArrowLeft/ArrowRight resize ±16 px (Shift ±64 px) clamped like drag, `aria-valuenow/valuemin/valuemax` + `aria-label` per divider (D7)
- [x] 4.2 App-level Escape handler closing Settings/Archive; added the missing `e.stopPropagation()` to the workspace-rename input's Escape branch so abandoning an edit doesn't also close the pane (D8)
- [x] 4.3 Palette swatches: `role="radio"` + `aria-checked` (replacing `aria-pressed`), ≥24 px hit area via an invisible `::after` extension without changing the 18 px visual (D9)

## 5. Focus-visible CSS sweep

- [x] 5.1 Extend the grouped `:focus-visible` rule to `.archive-row`, `.archive-tab`, `.archive-back`, `.archive-workspace-select`, `.btn-secondary`, `.graph-rail-more`, `.finishes-item`, `.season-recap-close`, `.split-pane-divider` (+ drag-affordance background on focused dividers); `.tree-row:focus-visible` (App.css:517) is now reachable
- [x] 5.2 Change `.archive-search:focus` and `.workspace-name-input:focus` to `:focus-visible` (text inputs match `:focus-visible` on any focus method, so click-to-edit affordances are unchanged)

## 6. Verification

- [x] 6.1 `bun run build` (strict TS) passes; full key-map functional pass against the real frontend in headless Chrome with mocked Tauri IPC (46 assertions, deterministic across runs: structure/ARIA, Tab entry, arrows + both no-wrap boundaries, Home/End, disclosure keys + persistence writes, Enter on grouping/content/dim rows, Space on a content row, follow-focus settle vs rapid skim, typeahead match/wrap/no-match, both dividers' resize + clamps, Escape on Archive, plus regression tests from the adversarial diff review: pointer chevron clicks never arm follow-focus, narrow-window divider grow-press never shrinks, aria-valuemax stays >= valuenow. Not harness-covered: Escape-on-Settings (same App.tsx handler branch as Archive; SettingsView needs unmocked IPC), the Tab-exit clause, and the no-whole-tree-re-render scenario, which holds by construction — focus is DOM-only with zero React state — per the verified design D2/D3) — harness at `~/.claude/jobs/0949336b/tmp/kbd-verify/verify.mjs`
- [x] 6.2 Screen-reader semantics verified at the computed-accessibility-tree layer (VoiceOver's input) via CDP: tree "Workspaces", correct levels 1-4, expanded/selected/disabled surfaced, groups present, separators named. Scoped here by decision (2026-06-10): the CDP evidence is accepted in lieu of a live VoiceOver listen, which needs a human/assistive-permissions session and remains an optional follow-up
- [x] 6.3 Verified via the harness: a cache event that removes the focused row falls focus back to the surviving ancestor; keyboard expansion toggles write the persisted collapse sets (`set_collapsed_tree_node_ids` observed), which is the same hydration path that already round-trips across restarts

## 7. Adversarial review fixes (3-lens diff review, verified findings)

- [x] 7.1 Gate follow-focus to keyboard-originated focus: pointer-induced focus (capture-phase mousedown flag) and the roving effect's programmatic restore no longer arm the 150 ms timer — a chevron click or watcher refresh can't navigate the detail pane (D4)
- [x] 7.2 Directional divider clamp: a keypress applies only when the clamped result moves in the pressed direction (no wrong-way snap at narrow windows); aria-valuemax floored at valuenow (D7)
- [x] 7.3 Palette radio group keyboard contract: checked swatch is the single tab stop; arrows move-and-select with wrap (D9)
- [x] 7.4 Escape in the workspace-rename input now truly abandons: abandoningRef stops the synchronous-blur commit of the stale draft (D11)
- [x] 7.5 Self-ticking RelativeTime leaf so memoization doesn't freeze mtime labels during quiet sessions (D10)
- [x] 7.6 Focus restore hoisted out of the row-vanished branch so a remount-with-same-ID also restores focus from <body>
