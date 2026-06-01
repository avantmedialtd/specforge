# Collapse the Tasks Node by Default

## Why

In the workspace tree, every artifact node under a change defaults to "expanded", and the Tasks node is the only one with a deep subtree: it expands into Section nodes, and each in-progress Section expands into its individual task rows. The current default rule (`spec-browser` → *Default Expansion of Tree Nodes* / *Auto-Collapse of Completed Task Groups*) only collapses the Tasks node once **every** task is complete. So for any in-progress change — the common case — expanding the change immediately dumps the whole task list into the tree. The `worktree-dev-slots` change (6 sections, 20 tasks, 0 done) expands to ~27 rows, dwarfing its one-line Proposal, Specs, and Design siblings.

The full task text is already available without the tree subtree: selecting the Tasks row renders the entire `tasks.md` in the detail pane, and per-task tree rows exist only for jump-to-task navigation. The auto-expanded task rows are therefore noise that crowds out in-progress work across other changes and makes the first-expand of a change lopsided.

This change makes the **Tasks artifact node collapse by default for every change, unconditionally** — not just when complete — so expanding a change yields a uniform four-row artifact list (Proposal, Specs, Design, a closed Tasks). The user drills into tasks by opting in, exactly as they already do for completed changes today.

## What Changes

- **The Tasks artifact node defaults to "collapsed" whenever it is collapsible** (its change has at least one section), regardless of task-completion state. Previously it defaulted collapsed only when 100% complete.
- **Section nodes are unchanged.** They keep their completion-based default (collapsed when all their tasks are complete, otherwise expanded), which only takes visible effect once the user expands the Tasks node.
- **Progress visibility is unchanged.** The Tasks row's meta slot still shows the task-progress meter (in progress) or the trailing `✓` (complete) whether the node is open or closed — collapsing hides the task *rows*, never the progress signal (governed by the untouched *Tasks Artifact Node Progress* requirement).
- **No persistence or migration work.** The two-set override model already tolerates a default flipping polarity: a user who expands the now-collapsed Tasks node records its ID in the `expanded` set; a stale ID left in the `collapsed` set from before this change is simply ignored (the spec already mandates this — *User Collapse State Persists Across Sessions*). Existing settings files load unchanged.

## Capabilities

### Modified Capabilities

- `spec-browser`: The *Default Expansion of Tree Nodes* requirement is amended so the Tasks artifact node defaults collapsed unconditionally (was: only when complete), and the *Auto-Collapse of Completed Task Groups* requirement is re-scoped so its completion-based rule governs Section nodes only, with the Tasks node stated to default collapsed regardless of completion.

## Impact

- **Spec:** two requirements modified in `openspec/specs/spec-browser/spec.md` — *Default Expansion of Tree Nodes* and *Auto-Collapse of Completed Task Groups* — with their scenarios updated. *Tasks Artifact Node Progress* is intentionally **not** modified: its meter/✓ behaviour is independent of expansion, and it already cross-references the auto-collapse requirement for the default.
- **Code:** a single edit in `src/components/WorkspaceTree.tsx` — `defaultIsOpenForTasksArtifact(change)` returns `false` unconditionally (or the `defaultOpen` for `kind === "tasks"` is set to `false`). The completion check it currently performs becomes dead and is removed. No other component changes; `defaultIsOpenForSection` and the meta-slot meter/✓ rendering are untouched.
- **No Rust, IPC, settings-schema, or persistence changes.** The `collapsedTreeNodeIds` / `expandedTreeNodeIds` settings keys and their semantics are unchanged.
- **Behaviour delta for users:** in-progress changes now open with a closed Tasks row instead of an exploded task list. Completed changes are visually unchanged (their Tasks node already collapsed). Any user who had manually expanded an in-progress Tasks node keeps that expansion (their override is in the `expanded` set after the first such click; pre-existing `collapsed`-set entries for the node are inert).

## Open Questions

- **Section default once Tasks is opened.** This change leaves Sections on their current completion-based default, so opening the Tasks node of an in-progress change still reveals its task rows (in-progress sections expanded). If the intent were "show section headers only," Sections would also need to default collapsed — explicitly out of scope here per the chosen direction (collapse the Tasks node only).
