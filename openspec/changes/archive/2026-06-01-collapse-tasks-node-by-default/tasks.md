# Tasks

## 1. Flip the Tasks-node default

- [x] 1.1 In `src/components/WorkspaceTree.tsx`, set `defaultOpen = kind !== "tasks"` in `ArtifactNode` so the Tasks artifact node defaults collapsed unconditionally (dropped the `totalTasks/completedTasks` completion check)
- [x] 1.2 Confirmed `defaultIsOpenForSection` and the Section subtree logic are untouched
- [x] 1.3 Confirmed the Tasks row's meta slot (progress meter / `✓`) still renders whether the node is open or closed (unchanged `meta` wiring in `ArtifactSubtree`)
- [x] 1.4 Removed the now-dead `defaultIsOpenForTasksArtifact` helper and updated the per-node-default comment; `bun run build` (tsc strict) is clean with no unused-symbol errors

## 2. Verify behaviour

- [x] 2.1 `bun run build` (`tsc --noEmit && vite build`) passes
- [x] 2.2 Ran the app via `bun run wt:dev` (slot 1 → 1430); screenshot confirms every Tasks node renders collapsed (`›`) with no task rows, while Proposal/Specs (expanded, showing capability children)/Design render as before — across both instances of this change and every Mushroom change
- [x] 2.3 The collapsed Tasks/instance rows still show the progress meter in the meta slot (screenshot); meter/✓ wiring is unchanged so the completed-state `✓` renders identically while collapsed
- [x] 2.4 Expand interaction is unchanged: `toggle(nodeId, defaultOpen=false)` routes to the `expanded` set (the same default-closed path completed Tasks/Sections already used), so clicking the Tasks caret reveals its sections per the Section default. Verified by the byte-unchanged toggle logic (only the default computation changed)
- [x] 2.5 Persistence mechanism (`expanded`/`collapsed` sets, debounced writes) is byte-unchanged; a Tasks node opened against the new closed default records its id in the `expanded` set and persists, per the existing *User Collapse State Persists Across Sessions* contract
- [x] 2.6 The app loaded the existing shared settings.json (which carries prior `collapsedTreeNodeIds`) and rendered cleanly with Tasks nodes collapsed — stale collapsed-set entries for Tasks nodes are inert (a default-closed node consults only the `expanded` set), causing no misrender

## 3. Sync spec deltas

- [x] 3.1 Confirmed the `spec-browser` deltas match the shipped behaviour (Tasks node collapsed unconditionally via `defaultOpen = kind !== "tasks"`; `defaultIsOpenForSection` unchanged; meta slot wiring unchanged)
- [x] 3.2 `openspec validate collapse-tasks-node-by-default --strict` passes
