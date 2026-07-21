## 1. Tokens

- [ ] 1.1 In `src/App.css`, add `--ok-tint` and `--ok-tint-strong` to the light `:root` token block near `--ok` / `--ok-strong`: `--ok-tint: rgba(16, 185, 129, 0.10)` and `--ok-tint-strong: rgba(16, 185, 129, 0.16)` (the `--ok` hue at the `--accent-tint` / `--accent-tint-strong` alphas), with a comment noting they mirror the accent-tint pair and are the completed-change wash.
- [ ] 1.2 Add the dark-scheme overrides in the `@media (prefers-color-scheme: dark)` `:root` block: `--ok-tint: rgba(52, 211, 153, 0.14)` and `--ok-tint-strong: rgba(52, 211, 153, 0.22)` (the dark `--ok` hue at the dark accent-tint alphas).

## 2. Completed-change wash

- [ ] 2.1 In `src/App.css`, extend the existing `.tree-row--complete` rule (currently `border-left-color: var(--ok-strong)`) to also set `background: var(--ok-tint)`, updating its comment to describe the additive wash (rail + wash, disc unchanged, selection still wins).
- [ ] 2.2 Add a `.tree-row--complete:hover { background: var(--ok-tint-strong); }` rule placed AFTER `.tree-row:hover` (so the equal-specificity tie resolves in its favour by source order), with a comment recording the specificity contract (mirrors `.tree-row.selected:hover`).
- [ ] 2.3 Confirm no change is needed in `src/components/WorkspaceTree.tsx` — verify `Row` still emits `tree-row--complete` for a completed two-line change row (singleton `InstanceNode` + `FlatChangeNode`, `complete` set from `allTasksDone`). If some prop threading turns out necessary, add it; otherwise record that no TSX change was required.

## 3. Spec sync

- [ ] 3.1 Confirm the delta at `openspec/changes/tint-completed-change-rows/specs/visual-identity/spec.md` matches what was built — the *Completed-State Styling* wash + hover-deepen + confinement-to-change-row, and the *Tree Row Selection Model* hover carve-out + selection-wins clause — adjusting the spec if implementation diverged.

## 4. Verification

- [ ] 4.1 Run the app on this worktree's dev slot (`bun run wt:dev`) and visually verify against a workspace with at least one completed change (this repo's own `openspec/` has archived completed changes, or register a workspace that does): an unselected completed change row shows the green wash + `--ok-strong` rail + completion disc; an in-progress change row is unchanged (workspace-colour rail, green meter, no wash).
- [ ] 4.2 Verify the four hover/selection cases in the running app: (a) completed + unselected + idle → `--ok-tint`; (b) completed + unselected + hover → `--ok-tint-strong` (NOT grey `--surface-2`); (c) completed + selected → `--accent-tint` wash + `--accent` bar (no green); (d) completed + selected + hover → `--accent-tint-strong`.
- [ ] 4.3 Verify in both light and dark schemes: the green wash is legible and subtle, `--text` (change name) and the change-id detail clear AA on the wash, and the `--ok-strong` completion disc still reads distinctly against the washed row.
- [ ] 4.4 Confirm the wash is confined to the change row — Sections, the Tasks artifact node, multi-instance child rows, and completed leaf tasks show no background wash (disc / green-struck-text only), and no completion animation was introduced.
- [ ] 4.5 `bun run build` passes (tsc strict + bundle).
