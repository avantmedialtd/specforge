# Expand Sidebar By Default

## Why

Today the sidebar auto-expands only the top-level workspace/repo rows; every logical change, instance, artifact, section, and task starts collapsed. Users must click in repeatedly before they can see anything useful, even though the four-artifact icons (✓/○) and per-task checkboxes were designed precisely so a glance at a fully-expanded tree functions as a status board. The current default fights the UI's own affordances.

## What Changes

- Every collapsible tree node defaults to **expanded** when first rendered, all the way down to individual tasks.
- The expansion-state model inverts: the tree tracks the set of node IDs the user has **collapsed** (instead of the set they have expanded). A node is open iff its ID is **not** in the set.
- The user's collapse choices **persist** across application restarts.
- Newly arrived nodes (logical changes, instances, artifacts surfaced by the watcher after initial mount) appear expanded automatically — no special handling needed; they're simply absent from the collapsed set.
- The one-shot `useEffect` in `WorkspaceTree.tsx` that seeded top-level workspace/repo IDs into the expanded set is removed (no longer needed under the inverted model).

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `spec-browser`: adds a new requirement governing the tree's default expansion state and the persistence of user-collapsed nodes. Existing requirements about which nodes are collapsible, what labels they show, and what clicking them does are unchanged.

## Impact

- **Frontend (`src/components/WorkspaceTree.tsx`)**: the `expanded: Set<string>` state and its seeding `useEffect` are replaced by a `collapsed: Set<string>` state, hydrated once on mount from settings and written back on every toggle. Every `isOpen` derivation in the file flips from `expanded.has(id)` to `!collapsed.has(id)`. Stale comment at line 316 ("Default to expanded …") becomes literally true and can be removed.
- **Frontend (`src/api.ts`, `src/hooks/`)**: a new wrapped IPC command for persisting the collapsed-ID set, and either a new hook or an extension of the existing settings access to expose it.
- **Rust shell (`crates/specforge/src/settings.rs`)**: `AppSettings` gains a `collapsed_tree_node_ids: Vec<String>` field with `#[serde(default)]` for forward/backward compatibility with existing settings files.
- **Rust shell (`crates/specforge/src/commands.rs`)**: a new `#[tauri::command]` handler that replaces the persisted ID set wholesale on each toggle. The full-set replace is cheap at this scale (a few hundred IDs at most) and keeps the IPC surface minimal.
- **TypeScript mirrors (`src/types.ts`)**: the `AppSettings` mirror gets the new field. No other type changes.
- **No watcher or core-crate changes.** The tree's expansion state lives entirely in the shell + frontend; `openspec-core` is unaffected.
- **User-visible behaviour shift on first launch after upgrade**: existing users will see a denser, fully-expanded sidebar on next launch. Their collapse choices from before were never persisted, so there is nothing to migrate; the inverted model simply starts with an empty collapsed set.
