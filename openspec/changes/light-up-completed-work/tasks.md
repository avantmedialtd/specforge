## 1. Completion glyph — the filled disc

- [ ] 1.1 In `src/components/icons.tsx`, add a filled-disc completion glyph — an `--ok`-fillable `<circle>` with a knocked-out check (polyline/path) in `--surface` — as a distinct component, leaving the existing outline `Check` for any non-completion use. Size it to sit in the `.row-meta` slot (~14–16px), clearly larger than a 4px status dot and carrying an interior check so it never reads as a dot.
- [ ] 1.2 In `src/App.css`, style the disc: `--ok` fill, `--surface` knocked-out check, `border-radius: 50%`, and **no** `box-shadow`/glow (the reserved-glow invariant). Confirm it reads distinctly from `.status-dot--ok`.
- [ ] 1.3 In `src/components/WorkspaceTree.tsx`, replace `<Check className="icon-checked" />` with the disc at all four completion sites: the Instance row status cluster, the Flat change row detail, the Tasks artifact node meta slot, and the Section node meta slot.

## 2. Completed-change rail

- [ ] 2.1 Thread a `complete` flag into the `Row` primitive so a completed two-line change row can override its rail; set it from `allTasksDone(change)` at the singleton `InstanceNode` and at `FlatChangeNode`.
- [ ] 2.2 In `Row`, when `complete` and the row is two-line, emit a `tree-row--complete` class that applies an `--ok` left rail *instead of* `tree-row--rail-{color}`.
- [ ] 2.3 In `src/App.css`, add the `tree-row--complete` `--ok` rail rule, positioned so the existing `.selected` accent-bar override still wins.

## 3. Completed leaf tasks

- [ ] 3.1 In `src/App.css`, change `.tree-row--struck .row-label` `color` from `--text-faint` to `--ok`; keep `text-decoration: line-through`.

## 4. Spec + verification

- [ ] 4.1 Spec captured in this change under `specs/visual-identity/spec.md` — *Outlined Chip Badges* modified to admit the second filled element; *Completed-State Styling* added.
- [ ] 4.2 In `bun tauri dev`, view a workspace with a fully-complete change and an in-progress one: confirm the completed change shows a green rail + filled green disc, completed sections and the Tasks node show the disc, completed tasks are green + struck, and the in-progress change is unchanged (workspace rail, green meter).
- [ ] 4.3 Select a completed change and confirm the selection accent bar overrides the green rail; deselect and confirm the green rail returns.
- [ ] 4.4 Toggle the OS between light and dark and confirm the disc, rail, and green tasks all resolve `--ok` correctly in both schemes and the disc's knocked-out check stays legible.
- [ ] 4.5 Confirm the disc carries no glow and is not mistakable for a 4px status dot, and that no row is washed on completion.
- [ ] 4.6 `bun run build` passes (tsc strict + bundle).
