## 1. RepoNode cleanup

- [x] 1.1 In `src/components/WorkspaceTree.tsx` `RepoNode`, compute `isEmpty = repo.active.length === 0` and remove the now-unused `totalActiveInstances` local.
- [x] 1.2 Pass `isLeaf={isEmpty}` to the top `<Row>`; gate `isExpanded` so it is only true when `!isEmpty && isOpen`; pass `onToggle={isEmpty ? undefined : () => toggle(nodeId, true)}`.
- [x] 1.3 Gate the inner `{isOpen && (...)}` block so the child container only renders when `!isEmpty && isOpen`; inside that container, drop the `repo.active.length === 0` branch (the "no active changes" placeholder `<Row>`) and the adjacent dead `{totalActiveInstances === 0 && null}` block, leaving only the `repo.active.map(...)` rendering.

## 2. FlatWorkspaceNode cleanup

- [x] 2.1 In `FlatWorkspaceNode`, compute `isEmpty = changes.length === 0` and apply the same `isLeaf` / `isExpanded` / `onToggle` treatment as in 1.2.
- [x] 2.2 Gate the inner `{isOpen && (...)}` block on `!isEmpty && isOpen`; inside, drop the `changes.length === 0` branch (the "no active changes" placeholder `<Row>`), leaving only the `changes.map(...)` rendering.

## 3. Verify in app

- [x] 3.1 Run `bun run build` to confirm `tsc --noEmit` passes (the strict `noUnusedLocals`/`noUnusedParameters` settings will catch any leftover `totalActiveInstances` or similar).
- [x] 3.2 Start `bun tauri dev` and walk the three relevant states for each top-level row type: empty (leaf row, badge `0`, no chevron), non-empty default-open, and the empty → non-empty transition (add an active change to a workspace; confirm the chevron appears and the badge advances).
- [x] 3.3 Confirm clicking an empty leaf row updates the tree's selected-node state with the same visual selection treatment a non-empty top-level row receives. The detail pane is intentionally unchanged for `repo`/`workspace` selections per the pre-existing `handleSelect` contract in `src/App.tsx`; that is not a regression.
