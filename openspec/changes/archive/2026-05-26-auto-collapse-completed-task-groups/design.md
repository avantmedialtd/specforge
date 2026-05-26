# Design

## Context

`WorkspaceTree.tsx` models expansion state as `collapsed: Set<string>` — the set of IDs the user has explicitly closed. A node is open iff its ID is absent from the set. The set persists in `AppSettings.collapsed_tree_node_ids` (see `crates/specforge/src/settings.rs:13`). The model was introduced by the `expand-sidebar-by-default` change (archived `2026-05-25-expand-sidebar-by-default`), which inverted the previous default-closed behaviour.

The existing model has one universal default: open. Every collapsible node behaves the same. The set encodes the user's deviations.

This change introduces a *per-node-type* default that flips based on runtime data:

- **Tasks artifact node**: default closed when `change.totalTasks > 0 && change.completedTasks === change.totalTasks`.
- **Section node**: default closed when `section.tasks.length > 0 && section.tasks.every(t => t.completed)`.
- **All other nodes**: default open (unchanged).

Once a node's default can be either open or closed, a single "deviations from default" set is no longer expressive enough — the user needs to be able to override in both directions (see Decisions below).

## Goals / Non-Goals

**Goals:**

- Completed Tasks artifacts and completed Sections render collapsed by default, drawing attention to in-progress work.
- The user can expand any auto-collapsed group with one click, and the expansion persists across restarts.
- Existing user-collapse choices on non-auto-collapsing nodes continue to behave exactly as before — no regression to the `expand-sidebar-by-default` semantics.
- The default re-evaluates on every render, so a section that becomes complete (or returns to in-progress because a new task was added) updates without any special seeding logic.
- Settings file remains backward-compatible; an upgrading user with no `expandedTreeNodeIds` field loads cleanly.

**Non-Goals:**

- Animation of the collapse/expand transition. Snap is fine for v1.
- Auto-collapsing the Change row or Instance row when all tasks done. These are primary navigation units; hiding the whole subtree by default feels too aggressive. (The Change row already shows a ✓ icon when complete — that's enough signal at that level.)
- A user-facing setting to disable auto-collapse. If it turns out to bite, we can revisit; ship with the rule on for everyone first.
- Garbage collection of orphaned IDs in either set. Already accepted as a non-issue under the existing model; the new set inherits the same treatment.
- Surfacing the auto-collapse state in any way other than the section ✓ glyph (no badges, no settings UI, no toast on first auto-close).
- Changes to `openspec-core`. Completion state is already in the IPC payloads (`completedTasks`, `totalTasks`, `Task.completed`); the auto-collapse derivation is a pure frontend concern.

## Decisions

### Two override sets, not a single combined map

Three implementation shapes exist for "track user overrides against a per-node default":

**(A) Single combined map `Map<id, "open" | "closed">`.**
Cleanest semantically — each entry is the user's explicit answer for that node. Requires migrating `AppSettings.collapsed_tree_node_ids` from `Vec<String>` to a struct-array, with a parser branch for the old shape. The migration is small but writes a new on-disk schema.

**(B) Two sets: `collapsed` (overrides default-open) and `expanded` (overrides default-closed).** — chosen.
Keeps the existing `collapsed_tree_node_ids` field exactly as it is; adds an `expanded_tree_node_ids: Vec<String>` field with `#[serde(default)]`. No migration; the old field's meaning is unchanged ("user-collapsed an otherwise-open node"). The toggle handler decides which set to mutate by reading the node's current default.

**(C) A "presentation-only" auto-collapse that doesn't persist user-expand overrides.**
Rejected. The moment a user expands an auto-collapsed section, that choice has to stick across re-renders (and ideally across restarts). Without persistence, the section would snap shut on the next watcher event. (C) ends up equivalent to (B) with worse UX, or equivalent to (A) with a hidden migration.

(B) wins on backward compatibility and on local reasoning — each set has one clear job, and the existing field semantics stay exactly as the `expand-sidebar-by-default` design described them.

### `defaultIsOpen` is a pure derivation over change data

A small helper in `WorkspaceTree.tsx`:

```ts
function defaultIsOpenForTasksArtifact(change: ChangeData): boolean {
    return !(change.totalTasks > 0 && change.completedTasks === change.totalTasks)
}

function defaultIsOpenForSection(section: Section): boolean {
    return !(section.tasks.length > 0 && section.tasks.every((t) => t.completed))
}
```

Called inside the existing `isOpen` derivation for the two affected node types. Pure functions over already-available props — no new context, no memoisation needed (re-computation is cheap and runs only on re-render). For every other node type, `isOpen` keeps today's shape: `!collapsed.has(nodeId)`.

### Toggle handler routes to the correct set

```ts
const toggle = (id: string, defaultOpen: boolean) => {
    if (defaultOpen) {
        setCollapsed((prev) => xor(prev, id))
    } else {
        setExpanded((prev) => xor(prev, id))
    }
}
```

Each call site that today calls `toggle(nodeId)` now calls `toggle(nodeId, defaultOpenAtThisNode)`. For nodes that aren't auto-collapse-eligible, `defaultOpenAtThisNode` is always `true` and behaviour is identical to today.

The choice of which set to mutate depends on the *default at the moment of click*. If a user expands a completed section (writes to `expanded`), then later the section transitions back to in-progress (default flips to open), the entry in `expanded` becomes inert — the default-open path consults `collapsed`. We leave the stale entry; it's cheap and self-correcting once the section re-completes.

### Section ✓ glyph

A completed section currently has nothing in its meta column. A collapsed-because-completed section would look identical to a collapsed-by-user in-progress section, which is confusing. The fix is a small ✓ at the right edge of the section row when every task in the section is done, regardless of the section's current expansion state.

This matches the existing patterns:

- the Change row gains a `Check` icon when `allTasksDone` (`WorkspaceTree.tsx:669`);
- the Tasks artifact label reads `Tasks (n/n)` which already conveys done-ness textually (`WorkspaceTree.tsx:768`).

We reuse the existing `Check` icon component (`src/components/icons`) with the `icon-checked` colour class — no new design token.

The Tasks artifact node does *not* get an additional ✓ glyph; its `(n/n)` label already does the job, and the existing artifact-present check icon (left side) would clash visually with a meta-column check.

### Hydration ordering

Both `getCollapsedTreeNodeIds` and `getExpandedTreeNodeIds` are issued in parallel on mount; the tree waits for both before flipping the `hydrated` flag. The brief unhydrated render still applies defaults from current props, which means completed sections appear collapsed during the gap (which is correct — that's the new default). Once hydration completes, a user's persisted `expanded` overrides snap any of those sections back open. The visible effect is the opposite direction of today's unhydrated → hydrated transition, but no worse.

### Persistence remains two independent debounced writes

We don't bundle the two sets into a single IPC call. They mutate at independent moments (a click against a default-open node never touches `expanded`), and the existing 150ms debounce already coalesces rapid toggles per-set. Two `useEffect` blocks of identical shape, one per set, keep the code easy to follow.

## Risks / Trade-offs

- **Risk**: A user checks off the last task in a section and the section vanishes beneath their cursor.
  → Mitigation: this is the intended behaviour. The visual jolt is small; the section title row remains, and the row's ✓ glyph signals "click to re-open if needed." A transition animation is a follow-up if users find it disorienting.

- **Risk**: A user expands a completed section, then a new task gets added (default flips to open). The `expanded` set still has the ID.
  → Mitigation: harmless. Once default-open, the path consults `collapsed`, which doesn't have the ID, so the section is open as expected. The stale `expanded` entry becomes live again if the section re-completes — at which point it expands the section, which matches what the user originally asked for.

- **Risk**: A user with many completed sections sees a near-empty subtree under the Tasks artifact and assumes data is missing.
  → Mitigation: the section title rows are still rendered (just collapsed); the ✓ glyph and the `Tasks (n/n)` label both convey "this is done, not missing." If real users report confusion, we can add an "expand done" affordance, but that's a follow-up.

- **Trade-off**: Two persisted sets instead of one. The on-disk size roughly doubles in the worst case but is still kilobytes at most; the read/write paths are duplicated but mechanical. Worth it to avoid the migration cost of the combined-map alternative.

- **Trade-off**: The `expanded` set's IDs are only relevant for nodes whose default is currently "closed". Entries for nodes whose default has flipped back to "open" are dead weight until/unless the default flips again. At realistic scale this is negligible; sweeping would cost more code than the bytes saved.
