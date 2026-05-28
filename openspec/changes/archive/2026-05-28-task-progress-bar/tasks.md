## 1. Progress-bar component

- [x] 1.1 Add a `TaskProgress` component (props: `completed: number`, `total: number`) — co-locate in `src/components/WorkspaceTree.tsx` unless it warrants its own file. It renders an outlined track containing a `--ok`-green fill whose width is `completed / total` (clamped to `[0, 1]`), with `role="progressbar"`, `aria-valuemin={0}`, `aria-valuemax={total}`, `aria-valuenow={completed}`, `aria-label="{completed} of {total} tasks"`, and a `title` of the same text for mouse hover
- [x] 1.2 Have the component render nothing when `total === 0` (no bar for changes with no parseable tasks) — callers may also guard, but the component should be safe on its own
- [x] 1.3 Do not render any inline digits inside the component — the count lives only in `title` + aria

## 2. Instance-row wiring (`InstanceNode`)

- [x] 2.1 In `InstanceNode` (`WorkspaceTree.tsx` ~:535), replace the `.row-progress` `<span>{completed}/{total}</span>` with `<TaskProgress>`, rendered only while `instance.change.artifacts.tasks && instance.change.totalTasks > 0 && !allTasksDone(instance.change)`
- [x] 2.2 Leave the existing trailing `✓` (`allTasksDone` → `<Check className="icon-checked" />`, ~:540) exactly as-is — it is now the sole signal at 100%, with the bar hidden
- [x] 2.3 Confirm meta-cluster order is preserved: status dot → bar (in progress) **or** `✓` (done) → mtime → divergence chip

## 3. Tasks artifact-node wiring (`ArtifactNode`)

- [x] 3.1 In `ArtifactSubtree` (~:848), change the Tasks `ArtifactNode` `label` from the conditional `Tasks (n/n)` string to a plain `"Tasks"`
- [x] 3.2 Give `ArtifactNode` an optional `meta?: ReactNode` prop and forward it to its `<Row meta=… />` (mirroring how `InstanceNode` / `FlatChangeNode` pass `meta`); pass `<TaskProgress>` as the Tasks node's meta when `change.artifacts.tasks && change.totalTasks > 0 && !allTasksDone(change)`
- [x] 3.3 Render a trailing `✓` (`<Check className="icon-checked" />`) in the Tasks node's meta when `allTasksDone(change)`, so the node still reads complete now that its label no longer carries `(n/n)`
- [x] 3.4 Verify the auto-collapse default for the Tasks node (`defaultIsOpenForTasksArtifact`) is unaffected — only the label/meta changed, not the expansion logic

## 4. CSS

- [x] 4.1 In `src/App.css`, repurpose/replace the `.row-progress` rule as the bar: a fixed-width (~56px) outlined track (`border: var(--border-width) solid var(--border)`, transparent background, `border-radius: var(--radius-sm)`, a small fixed height ~4–6px) with an inner fill element in `--ok` and `transition: width` for the toggle nudge
- [x] 4.2 Wrap the width transition in `@media (prefers-reduced-motion: reduce)` and disable it there
- [x] 4.3 Confirm the bar's height/baseline does not disturb the row's vertical rhythm (`text-xs` single-line rows) — adjust vertical alignment so it sits centered in the meta cluster

## 5. Spec sync (applied at archive time via `openspec archive`)

- [x] 5.1 Apply the `spec-browser` delta from `openspec/changes/task-progress-bar/specs/spec-browser/spec.md` (modify *Instance Row Chrome*, *Reactive Updates from Filesystem*, *Auto-Collapse of Completed Task Groups*, *Completed Section Row Shows a Completion Glyph*, *Change-Row Completion Glyph*)
- [x] 5.2 Apply the `visual-identity` delta from `openspec/changes/task-progress-bar/specs/visual-identity/spec.md` (modify *Typography System* and *Outlined Chip Badges*; add *Task Progress Meter*)

## 6. Manual verification

- [x] 6.1 Run `bun tauri dev`. On an in-progress change, confirm the instance row shows a green-filled outlined bar (no digits) whose fill matches the completion ratio; hover it and confirm the tooltip reads "N of M tasks"
- [x] 6.2 Confirm the Tasks artifact row shows the same bar in its meta slot and its label is plain `Tasks` (no `(n/n)`)
- [x] 6.3 Toggle a task to complete in a `tasks.md` on disk; confirm both bars widen within the watcher debounce window and the fill animates (and does not animate under a reduced-motion OS setting)
- [x] 6.4 Complete every task in a change; confirm both the instance row and the Tasks artifact row drop the bar and show the trailing `✓`, and the Tasks node auto-collapses as before
- [x] 6.5 Confirm a change whose `tasks.md` parses zero tasks shows no bar on either row
- [x] 6.6 Tab/inspect the bar with an a11y tool (or read the DOM) and confirm `role="progressbar"` with correct `aria-valuenow` / `aria-valuemax` / `aria-label`

## 7. Build check

- [x] 7.1 Run `bun run build` and confirm `tsc --noEmit` plus the Vite build succeed (the new `meta` prop on `ArtifactNode` must type-check under `noUnusedLocals` / `noUnusedParameters`)
- [x] 7.2 Run `cargo test` and confirm no Rust tests are affected (this is a frontend-only change; the run confirms no accidental Rust edits)
