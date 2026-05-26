# Tasks

## 1. Rust settings: add the expanded-ID field

- [x] 1.1 In `crates/specforge/src/settings.rs`, add an `expanded_tree_node_ids: Vec<String>` field to `AppSettings`, annotated with `#[serde(default)]` so existing `settings.json` files (lacking the field) deserialize cleanly.
- [x] 1.2 Update `AppSettings::default()` to initialise the new field to an empty `Vec`.
- [x] 1.3 Add a `set_expanded_tree_node_ids(&self, ids: Vec<String>) -> io::Result<()>` method to `SettingsStore`, mirroring the existing `set_collapsed_tree_node_ids` shape: lock, replace, snapshot-clone, drop guard, persist via `save`.

## 2. Rust commands: expose the new persistence pair

- [x] 2.1 In `crates/specforge/src/commands.rs`, add a `get_expanded_tree_node_ids` command returning `Result<Vec<String>, String>` that reads from `SettingsStore::snapshot()`.
- [x] 2.2 In the same file, add a `set_expanded_tree_node_ids` command taking `ids: Vec<String>` that calls the new setter on `SettingsStore`.
- [x] 2.3 In `crates/specforge/src/lib.rs`, register both new commands in the `invoke_handler!` macro alongside the existing `get_collapsed_tree_node_ids` / `set_collapsed_tree_node_ids` entries.
- [x] 2.4 Run `cargo test` and confirm the workspace still compiles and existing tests pass (no new Rust tests are needed — the setter is a thin pass-through).

## 3. Frontend API wrappers

- [x] 3.1 In `src/api.ts`, add `getExpandedTreeNodeIds(): Promise<string[]>` wrapping `invokeLogged<string[]>("get_expanded_tree_node_ids")`.
- [x] 3.2 In the same file, add `setExpandedTreeNodeIds(ids: string[]): Promise<void>` wrapping `invokeLogged<void>("set_expanded_tree_node_ids", { ids })`.

## 4. Frontend default derivation

- [x] 4.1 In `src/components/WorkspaceTree.tsx`, add a top-level helper `defaultIsOpenForTasksArtifact(change: ChangeData): boolean` returning `!(change.totalTasks > 0 && change.completedTasks === change.totalTasks)`.
- [x] 4.2 In the same file, add a top-level helper `defaultIsOpenForSection(section: Section): boolean` returning `!(section.tasks.length > 0 && section.tasks.every((t) => t.completed))`.
- [x] 4.3 Keep all other nodes' default as `true` (open) — do not add per-type helpers for them.

## 5. Frontend state: add the second set and route the toggle

- [x] 5.1 Add a second `useState<Set<string>>(new Set())` for `expanded` alongside the existing `collapsed` state in `WorkspaceTree`.
- [x] 5.2 Change the `toggle` signature to `(id: string, defaultOpen: boolean)`. When `defaultOpen` is true, xor the id into `collapsed` (existing behaviour). When false, xor the id into `expanded`.
- [x] 5.3 Update the `NodeProps` interface (or equivalent) and every descendant's `toggle` call so the call passes the right `defaultOpen` for the node being toggled. For non-auto-collapsing nodes this is always `true`.
- [x] 5.4 In `ArtifactNode` for `kind === "tasks"`: compute `defaultOpen = defaultIsOpenForTasksArtifact(change)`. Derive `isOpen` as `defaultOpen ? !collapsed.has(nodeId) : expanded.has(nodeId)`. Pass `defaultOpen` into `toggle`.
- [x] 5.5 In `SectionNode`: compute `defaultOpen = defaultIsOpenForSection(section)`. Derive `isOpen` the same way. Pass `defaultOpen` into `toggle`.
- [x] 5.6 For every other node type that calls `toggle`, pass `true` as the second argument explicitly so the routing is obvious at the call site (no relying on a default parameter).

## 6. Hydration + debounced persistence for the second set

- [x] 6.1 In the hydration `useEffect`, fire both `getCollapsedTreeNodeIds()` and `getExpandedTreeNodeIds()` in parallel (`Promise.all`). Seed `collapsed` and `expanded` together. Set the `hydrated` flag only after both resolve.
- [x] 6.2 Add a second debounced-persistence `useEffect` watching `expanded`, mirroring the existing one for `collapsed`: 150ms quiet period, then `setExpandedTreeNodeIds([...expanded])`. Gate on `hydrated` so the initial seed doesn't write back. Cancel timeout on unmount and on re-run.
- [x] 6.3 Confirm that error paths (`getExpandedTreeNodeIds` rejects) still flip `hydrated` to `true` so the tree becomes interactive — match the existing collapsed-side error handling.

## 7. UI: ✓ glyph on completed Section rows

- [x] 7.1 In `SectionNode`, compute `allTasksDone = section.tasks.length > 0 && section.tasks.every((t) => t.completed)`.
- [x] 7.2 Pass a `meta` prop to the `Row` containing `<Check className="icon-checked" />` when `allTasksDone`, otherwise nothing (preserve current "no meta" behaviour).
- [x] 7.3 Verify the ✓ renders identically to the existing `Check` icon usage in `FlatChangeNode` (line 669) — no new CSS classes.

## 8. Verification

- [x] 8.1 Run `bun run build` and confirm `tsc --noEmit` passes (TypeScript strict mode, `noUnusedLocals`, `noUnusedParameters`).
- [x] 8.2 Run `cargo test` after the frontend work to confirm nothing crossed crate boundaries unexpectedly.
- [x] 8.3 Start `bun tauri dev` and visually verify with a change that has at least one fully-complete section and one partially-complete section:
    - (a) the complete section renders collapsed by default with a ✓ in its meta column;
    - (b) the partial section renders expanded by default with no ✓;
    - (c) clicking the complete section's chevron expands it and persists across restart;
    - (d) clicking the partial section's chevron collapses it (writes to the original `collapsed` set) and persists across restart;
    - (e) a Tasks artifact whose change has `(n/n)` renders collapsed; with `(n/m)` it renders expanded;
    - (f) adding a new unchecked task to a complete section (edit `tasks.md` on disk) causes the section to re-expand on the next watcher event without losing the user's other collapse state.
- [ ] 8.4 Delete `~/Library/Application Support/com.avantmedia.specforge/settings.json` and relaunch to confirm the "no persisted state" branch loads cleanly with both sets empty.
- [ ] 8.5 Edit `settings.json` by hand to remove the `expandedTreeNodeIds` field and relaunch to confirm `#[serde(default)]` lets the older shape load without error.
- [x] 8.6 With a change whose every section is complete and whose Tasks artifact is therefore `(n/n)`: confirm both the Tasks artifact node and every section beneath it collapse by default, and that the Change row itself remains expanded (it is not auto-collapse-eligible).
