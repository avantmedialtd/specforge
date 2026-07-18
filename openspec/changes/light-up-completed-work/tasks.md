## 1. Completion glyph — the filled disc

- [x] 1.1 In `src/components/icons.tsx`, add a filled-disc completion glyph — an `--ok-strong`-fillable `<circle>` with a knocked-out check (polyline) in `--surface` — as a distinct component (`CompletionMark`), leaving the existing outline `Check` for any non-completion use. Sized to sit in the `.row-meta` slot (15px), clearly larger than a 4px status dot and carrying an interior check so it never reads as a dot.
- [x] 1.2 In `src/App.css`, style the disc: `--ok-strong` fill, `--surface` knocked-out check, and **no** `box-shadow`/glow (the reserved-glow invariant). Reads distinctly from `.status-dot--ok`.
- [x] 1.3 In `src/components/WorkspaceTree.tsx`, replace `<Check className="icon-checked" />` with the disc at all four completion sites: the Instance row status cluster, the Flat change row detail, the Tasks artifact node meta slot, and the Section node meta slot.

## 2. Completed-change rail

- [x] 2.1 Thread a `complete` flag into the `Row` primitive so a completed two-line change row can override its rail; set from `allTasksDone(change)` at the singleton `InstanceNode` and at `FlatChangeNode`.
- [x] 2.2 In `Row`, when `complete` and the row is two-line, emit a `tree-row--complete` class that applies an `--ok-strong` left rail *instead of* `tree-row--rail-{color}` (mutually exclusive).
- [x] 2.3 In `src/App.css`, add the `tree-row--complete` `--ok-strong` rail rule; the existing `.selected` accent-bar override (higher specificity) still wins.

## 3. Completed leaf tasks

- [x] 3.1 In `src/App.css`, change `.tree-row--struck .row-label` `color` from `--text-faint` to `--ok-strong`; keep `text-decoration: line-through`.

## 4. Token + verification

- [x] 4.1 Add the `--ok-strong` token (light `#047857` ≈5.3:1 on white, dark `#34d399`) — a deep, AA-readable foreground "done" green, distinct from `--ok` (the progress-meter fill). Spec captured under `specs/visual-identity/spec.md` (modified *Outlined Chip Badges*; added *Completed-State Styling*).
- [x] 4.2 Verified the rendered styling in both schemes: the completed change shows a green rail + filled green disc, completed tasks are green + struck, and the in-progress change is unchanged (workspace rail, green meter). (Verified via the running `wt:dev` app + an isolated real-token render harness; the live desktop-window screenshot was blocked by a shared web-server port held by another running instance.)
- [x] 4.3 Confirmed `.tree-row.selected` (0,2,0) overrides `.tree-row--complete` (0,1,0), so a selected completed change shows the `--accent` bar and an unselected one shows the `--ok-strong` rail (specificity-verified in review).
- [x] 4.4 Confirmed the disc, rail, and green tasks resolve correctly in both light and dark, and the disc's knocked-out check stays legible (light ~5.3:1, dark high-contrast).
- [x] 4.5 Confirmed the disc carries no glow and is not mistakable for a 4px status dot, and that no row is washed on completion.
- [x] 4.6 `bun run build` passes (tsc strict + bundle).
