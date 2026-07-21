## 1. Tokens

- [x] 1.1 In `src/App.css`, add `--ok-tint` and `--ok-tint-strong` to the light `:root` token block near `--ok` / `--ok-strong`: `--ok-tint: rgba(16, 185, 129, 0.10)` and `--ok-tint-strong: rgba(16, 185, 129, 0.16)` (the `--ok` hue at the `--accent-tint` / `--accent-tint-strong` alphas), with a comment noting they mirror the accent-tint pair and are the completed-change wash.
- [x] 1.2 Add the dark-scheme overrides in the `@media (prefers-color-scheme: dark)` `:root` block: `--ok-tint: rgba(52, 211, 153, 0.14)` and `--ok-tint-strong: rgba(52, 211, 153, 0.22)` (the dark `--ok` hue at the dark accent-tint alphas).

## 2. Completed-change wash

- [x] 2.1 In `src/App.css`, extend the existing `.tree-row--complete` rule (currently `border-left-color: var(--ok-strong)`) to also set `background: var(--ok-tint)`, updating its comment to describe the additive wash (rail + wash, disc unchanged, selection still wins).
- [x] 2.2 Add a `.tree-row--complete:hover { background: var(--ok-tint-strong); }` rule placed AFTER `.tree-row:hover` (so the equal-specificity tie resolves in its favour by source order), with a comment recording the specificity contract (mirrors `.tree-row.selected:hover`).
- [x] 2.3 Confirm no change is needed in `src/components/WorkspaceTree.tsx` — verify `Row` still emits `tree-row--complete` for a completed two-line change row (singleton `InstanceNode` + `FlatChangeNode`, `complete` set from `allTasksDone`). If some prop threading turns out necessary, add it; otherwise record that no TSX change was required. **No TSX change required** — `railClass` (WorkspaceTree.tsx:655-657) emits `tree-row--complete` for `detail != null && complete`; both `InstanceNode` (:1092) and `FlatChangeNode` (:1355) pass `complete={allTasksDone(...)}` on two-line rows.

## 3. Spec sync

- [x] 3.1 Confirm the delta at `openspec/changes/tint-completed-change-rows/specs/visual-identity/spec.md` matches what was built — the *Completed-State Styling* wash + hover-deepen + confinement-to-change-row, and the *Tree Row Selection Model* hover carve-out + selection-wins clause — adjusting the spec if implementation diverged.

## 4. Verification

- [x] 4.1 Visually verify a completed vs in-progress change row. Verified via an **isolated render harness** that inlines the real `src/App.css` and reproduces the exact tree-row DOM (the live Tauri app can't show a completed row here — this repo has zero *active* changes, and completion is task-derived on active changes only; the prior `light-up-completed-work` change used the same harness fallback). Confirmed: the unselected completed row shows the green wash + `--ok-strong` rail + completion disc; the in-progress row is unchanged (workspace-colour rail `rgb(52,178,106)`, green meter, transparent bg).
- [x] 4.2 Four hover/selection cases verified by computed-style reads on the real CSS (deterministic), corroborated by CSSOM source order and two real mouse hovers: (a) completed idle → `rgba(16,185,129,0.1)` = `--ok-tint`; (b) completed + hover → `rgba(16,185,129,0.16)` = `--ok-tint-strong` (real `:hover`, NOT `--surface-2`); (c) completed + selected → `rgba(79,91,217,0.1)` = `--accent-tint` (accent wins); (d) selected + complete + hover → `rgba(79,91,217,0.16)` = `--accent-tint-strong` (real `:hover`, no green). Source order `.tree-row:hover` → `.tree-row--complete:hover` confirms the equal-specificity tie resolves to the completion hover.
- [x] 4.3 Both schemes verified in the harness (light + dark side by side): the green wash is subtle, the change name / `main` chip / changeid remain legible on it (alphas matched to the accepted `--accent-tint`), and the `--ok-strong` completion disc reads distinctly against the washed row. Dark values confirmed: completed idle `rgba(52,211,153,0.14)`, selected `rgba(124,140,255,0.14)`.
- [x] 4.4 Confinement confirmed — child artifact rows, the Section/Tasks rows, and completed struck leaf tasks all read `background: rgba(0,0,0,0)` (transparent); their completion shows only as the disc (Section / Tasks) or green struck text (leaf tasks). No wash leaks past the change row. No animation introduced — the change is static CSS only.
- [x] 4.5 `bun run build` passes (tsc strict + vite bundle; only the pre-existing >500 kB chunk-size warning).
