# Auto-Collapse Completed Task Groups

## Why

The sidebar shows the full task list for every change by default. As changes near completion, the still-open task groups keep displaying rows the user no longer needs to act on — done work clutters the view of remaining work. The same goes at the Tasks artifact level: a change with `Tasks (8/8)` still expands its full subtree by default, even though every line is checked.

We want completed task groups to auto-collapse so the user's eye is drawn to what's still in progress, while keeping the ability to expand any done group on demand and persist that expansion.

## What Changes

- The Tasks artifact node SHALL render collapsed by default when its change has at least one task and every task is complete.
- A Section node SHALL render collapsed by default when its section has at least one task and every task in it is complete.
- The expansion-state model gains a second persisted set, `expandedTreeNodeIds`. A node whose computed default is "closed" is rendered open iff its ID is in the `expanded` set; a node whose computed default is "open" is rendered closed iff its ID is in the existing `collapsed` set (unchanged behaviour for non-auto-collapsing nodes).
- A completed Section row SHALL display a small ✓ in its meta column, so a closed auto-collapsed section is visually distinguishable from a closed in-progress section that the user collapsed manually.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `spec-browser`: extends the default-expansion model so completed Tasks artifacts and completed Sections are collapsed by default, and adds a persisted "expanded overrides" set so the user can explicitly expand such nodes. The user-collapse semantics for nodes whose default is still "open" are unchanged.

## Impact

- **Frontend (`src/components/WorkspaceTree.tsx`)**: the single `collapsed: Set<string>` becomes a pair (`collapsed`, `expanded`), hydrated together on mount and persisted together on toggle. A helper `defaultIsOpen(node)` derives the per-node default from completion state. Section rows surface a ✓ in their meta column when all tasks done. The toggle handler routes the click to the correct set based on which side of the default the user is acting against.
- **Frontend (`src/api.ts`)**: a second IPC pair (`getExpandedTreeNodeIds` / `setExpandedTreeNodeIds`) wrapping the new Rust commands.
- **Rust shell (`crates/specforge/src/settings.rs`)**: `AppSettings` gains an `expanded_tree_node_ids: Vec<String>` field with `#[serde(default)]`. A second `set_expanded_tree_node_ids` setter mirrors the existing collapsed one.
- **Rust shell (`crates/specforge/src/commands.rs`, `crates/specforge/src/lib.rs`)**: two new `#[tauri::command]` handlers, registered in `invoke_handler!`.
- **TypeScript mirrors (`src/types.ts`)**: no public type touched — the new field is local to the settings file shape, not crossing the rendered-tree boundary.
- **No watcher or core-crate changes.** Auto-collapse is a pure UI derivation over `change.completedTasks`, `change.totalTasks`, and per-task `completed` flags that already cross the IPC boundary.
- **User-visible behaviour shift on first launch after upgrade**: existing users will see completed sections and completed Tasks artifacts collapsed on next launch. Anyone who had explicitly collapsed an in-progress section that has since completed sees no change (still closed). The new `expanded` set starts empty; the user re-expands what they want and that choice persists.
