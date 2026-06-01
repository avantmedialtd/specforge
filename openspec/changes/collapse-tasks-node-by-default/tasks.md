# Tasks

## 1. Flip the Tasks-node default

- [ ] 1.1 In `src/components/WorkspaceTree.tsx`, make the Tasks artifact node default to collapsed unconditionally — `defaultIsOpenForTasksArtifact` returns `false` (or set `defaultOpen` for `kind === "tasks"` to `false`), dropping the `totalTasks/completedTasks` completion check
- [ ] 1.2 Confirm `defaultIsOpenForSection` and the Section subtree logic are untouched
- [ ] 1.3 Confirm the Tasks row's meta slot (progress meter / `✓`) still renders whether the node is open or closed (unchanged `meta` wiring in `ArtifactSubtree`)
- [ ] 1.4 Remove any now-dead helper/branch left by the completion check so `noUnusedLocals`/`noUnusedParameters` stay clean

## 2. Verify behaviour

- [ ] 2.1 `bun run build` (`tsc --noEmit && vite build`) passes
- [ ] 2.2 With `bun tauri dev`, expand an in-progress change and confirm its Tasks node is collapsed by default while Proposal/Specs/Design render expanded
- [ ] 2.3 Confirm the collapsed Tasks row still shows the progress meter; complete all tasks (or pick a completed change) and confirm the row shows the `✓` while still collapsed
- [ ] 2.4 Click the Tasks caret to expand; confirm sections appear per the Section default (in-progress expanded with task rows, completed collapsed with `✓`)
- [ ] 2.5 Quit and relaunch; confirm a Tasks node the user expanded stays expanded (override in the `expanded` set persists)
- [ ] 2.6 Confirm an existing settings file with prior `collapsedTreeNodeIds` entries for Tasks nodes loads cleanly and those inert entries cause no misrender

## 3. Sync spec deltas

- [ ] 3.1 Confirm the `spec-browser` deltas match the shipped behaviour (Tasks node collapsed unconditionally; Sections unchanged; meta slot unchanged)
- [ ] 3.2 `openspec validate collapse-tasks-node-by-default --strict` passes
