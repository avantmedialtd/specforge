# Tasks

## 1. Rust settings: add the collapsed-ID field

- [x] 1.1 In `crates/specforge/src/settings.rs`, add a `collapsed_tree_node_ids: Vec<String>` field to `AppSettings`, annotated with `#[serde(default)]` so existing `settings.json` files (lacking the field) deserialize cleanly.
- [x] 1.2 Update `AppSettings::default()` to initialise the new field to an empty `Vec`.
- [x] 1.3 Add a `set_collapsed_tree_node_ids(&self, ids: Vec<String>) -> io::Result<()>` method to `SettingsStore`, mirroring the existing `set_notifications_enabled` shape: lock, replace, snapshot-clone, drop guard, persist via `save`.

## 2. Rust commands: expose the new persistence pair

- [x] 2.1 In `crates/specforge/src/commands.rs`, add a `get_collapsed_tree_node_ids` command returning `Result<Vec<String>, String>` that reads from `SettingsStore::snapshot()`.
- [x] 2.2 In the same file, add a `set_collapsed_tree_node_ids` command taking `ids: Vec<String>` that calls the new setter on `SettingsStore`.
- [x] 2.3 In `crates/specforge/src/lib.rs`, register both new commands in the `invoke_handler!` macro alongside the existing settings commands.
- [x] 2.4 Run `cargo test` and confirm the workspace still compiles and existing tests pass (no new Rust tests are needed — the setter is a thin pass-through).

## 3. Frontend API wrappers

- [x] 3.1 In `src/api.ts`, add `getCollapsedTreeNodeIds(): Promise<string[]>` wrapping `invokeLogged<string[]>("get_collapsed_tree_node_ids")`.
- [x] 3.2 In the same file, add `setCollapsedTreeNodeIds(ids: string[]): Promise<void>` wrapping `invokeLogged<void>("set_collapsed_tree_node_ids", { ids })`.

## 4. Frontend state inversion in `WorkspaceTree.tsx`

- [x] 4.1 Rename the `expanded` state to `collapsed`; keep it as `useState<Set<string>>(new Set())`.
- [x] 4.2 Flip every `expanded.has(id)` reference to `!collapsed.has(id)` (these drive `isOpen` for each node type — Repo, LogicalChange, Instance, ArtifactNode, SectionNode, FlatWorkspaceNode, FlatChangeNode).
- [x] 4.3 Rewrite `toggle` to operate on `collapsed`: if the set has the id, delete it; otherwise add it.
- [x] 4.4 Rename the `expanded` prop / `Set<string>` parameter on every helper component (`RepoNode`, `LogicalChangeRow`, `InstanceNode`, `DisclosureGroup`, `FlatWorkspaceNode`, `FlatChangeNode`, `ArtifactSubtree`, `ArtifactNode`, `SectionNode`) to `collapsed`, and update each component's internal `isOpen` derivation accordingly.

## 5. Hydration + debounced persistence

- [x] 5.1 Add a `useEffect` that runs once on `WorkspaceTree` mount, calls `getCollapsedTreeNodeIds()`, and seeds the `collapsed` state with the result. While the call is in flight, the tree is fine to render with an empty collapsed set (i.e. everything open) — no loading gate.
- [x] 5.2 Add a debounced persistence `useEffect` that watches `collapsed` and, after a 150 ms quiet period, calls `setCollapsedTreeNodeIds([...collapsed])`. The initial mount-hydration cycle must not trigger a write back (e.g., gate the effect on a "has hydrated" flag).
- [x] 5.3 Ensure the persistence effect cleanly cancels its pending timeout on unmount and on every re-run, so rapid toggles coalesce to a single write.

## 6. Cleanup

- [x] 6.1 Delete the `useEffect` at `WorkspaceTree.tsx:76-88` that seeded top-level repo/workspace IDs into the `expanded` set — under the inverted model nothing seeds anything.
- [x] 6.2 Remove the misleading second sentence of the comment at `WorkspaceTree.tsx:316` ("Promotion (singleton → multi) is handled by useEffect below.") — there is no such effect. Keep the first sentence ("Default to expanded …") since it now reflects actual behaviour, or replace with a one-liner referencing the inverted model.
- [x] 6.3 Re-read the file end-to-end and remove any stale comment about "auto-expanding on first sight" or similar — the inverted model makes those obsolete.

## 7. Verification

- [x] 7.1 Run `bun run build` and confirm `tsc --noEmit` passes (TypeScript strict mode, `noUnusedLocals`, `noUnusedParameters`).
- [x] 7.2 Run `cargo test` again after the frontend work to confirm nothing crossed crate boundaries unexpectedly.
- [x] 7.3 Start `bun tauri dev` and visually verify: (a) fresh tree shows every change fully expanded down to tasks, (b) collapsing any node persists across an app restart, (c) re-expanding it persists too, (d) adding a new change directory on disk causes the new rows to appear expanded (no extra click needed).
- [x] 7.4 Delete `~/Library/Application Support/com.avantmedia.specforge/settings.json` (or the platform-equivalent path) and relaunch to confirm the "no persisted state" branch loads cleanly with an empty collapsed set.
- [x] 7.5 Edit `settings.json` by hand to remove the `collapsedTreeNodeIds` field and relaunch to confirm `#[serde(default)]` lets the older shape load without error.
