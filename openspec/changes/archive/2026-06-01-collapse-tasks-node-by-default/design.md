# Design — Collapse the Tasks Node by Default

## Context

The workspace tree's default-expansion logic lives entirely in `src/components/WorkspaceTree.tsx` and is governed by two `spec-browser` requirements: *Default Expansion of Tree Nodes* and *Auto-Collapse of Completed Task Groups*. Today exactly two node types have a completion-dependent default:

```
defaultIsOpenForTasksArtifact(change) = !(totalTasks > 0 && completedTasks === totalTasks)
defaultIsOpenForSection(section)      = !(tasks.length > 0 && tasks.every(completed))
```

Both default open while work is in progress and collapse on completion. Every other node defaults open. Rendered state combines the default with two persisted override sets via `isOpen = defaultOpen ? !collapsed.has(id) : expanded.has(id)`.

The decision (captured via explore): **collapse the Tasks artifact node for every change, unconditionally; leave Sections as-is.**

## Decisions

### Decision 1 — Tasks node defaults collapsed unconditionally

`defaultIsOpenForTasksArtifact` becomes `false` for any collapsible Tasks node, dropping the completion check. The node's `defaultOpen` is therefore always `false`, so its overrides live in the `expanded` set (a user opens it to opt in). This is the minimal expression of the chosen behaviour and removes a branch rather than adding one.

**Why unconditional (not "only in-progress"):** the in-progress case was the only one still defaulting open; flipping just that case yields a node that is *always* collapsed anyway. Stating it unconditionally is the simpler, truer rule and makes the completed-vs-in-progress distinction irrelevant to expansion (it survives only in the meta-slot meter/✓).

### Decision 2 — Sections unchanged

`defaultIsOpenForSection` is untouched. Once a user expands the Tasks node, in-progress sections still reveal their task rows and completed sections stay collapsed with their ✓. This keeps the opt-in drill-down immediately useful (you see actual tasks, not a second wall of closed headers) and confines the change to one node type. Collapsing Sections too was considered and explicitly rejected as the chosen direction.

### Decision 3 — Progress signal stays; only rows are hidden

The Tasks row's meta slot (task-progress meter or ✓) is rendered by the `Row` `meta` prop independently of `isOpen`, and is governed by the separate *Tasks Artifact Node Progress* requirement. Collapsing the node hides only its descendant rows, so at-a-glance progress per change is preserved in the tree. This is why *Tasks Artifact Node Progress* is left unmodified.

### Decision 4 — No migration; rely on the existing override model

The two-set design already specifies that "a persisted ID in one set whose node's default has since flipped to the other polarity SHALL be ignored" (*User Collapse State Persists Across Sessions*). When the Tasks node default flips open→closed:

- A user who previously **collapsed** an in-progress Tasks node has its ID in `collapsed`; post-change that set is not consulted for a default-closed node, so the node renders collapsed (their intent, and the new default — no conflict).
- A user who previously **expanded** a completed Tasks node has its ID in `expanded`; post-change the node is default-closed and `expanded.has(id)` keeps it open (preference honoured).

No settings-schema change, no data migration, no GC of stale IDs (the spec already permits inert entries).

## Spec surgery

Two requirements are reproduced with edits (MODIFIED deltas):

- ***Default Expansion of Tree Nodes*** — the Tasks-node clause moves from the completion-dependent group to its own unconditional rule; the "first launch" and "new change" scenarios are updated to assert the Tasks node renders collapsed regardless of completion; a "User expand overrides the collapsed Tasks node" scenario is added; the existing Section scenarios are retained.
- ***Auto-Collapse of Completed Task Groups*** — re-scoped to Section nodes only; adds the explicit statement that the Tasks node defaults collapsed regardless of completion; drops the now-false "Tasks artifact stays expanded when partially complete" scenario; adds "Tasks artifact node defaults collapsed regardless of completion" and "Completing the last task in a change does not change its Tasks node expansion" (it was already collapsed; only the meta swaps meter→✓).

## Risks / trade-offs

- **One extra click to reach a task row.** Acceptable: the full task list is one click away in the detail pane (select the Tasks row), and per-task tree rows are a navigation convenience, not the only access path.
- **Users accustomed to seeing tasks on expand** will notice the change. Mitigated by the persistent override (expand once; it sticks) and the always-visible progress meter.
