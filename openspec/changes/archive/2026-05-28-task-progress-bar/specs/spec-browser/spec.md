## MODIFIED Requirements

### Requirement: Instance Row Chrome

Each `ChangeInstance` row SHALL display the instance's branch name as its primary label, falling back to the worktree path's basename when the branch is not known (detached HEAD, bare worktree, non-git workspace re-using this rendering path). The row SHALL additionally display a task-progress meter (see the *Task Progress Meter* requirement in the `visual-identity` capability) and the relative modification time. Divergence labels (when present) and the active indicator (when present) attach to the row alongside these elements.

#### Scenario: Instance label uses branch when available

- **WHEN** an instance's worktree is on a named branch
- **THEN** the row's primary label is the branch name

#### Scenario: Instance label falls back to path basename

- **WHEN** an instance's worktree is not on a named branch (detached HEAD or no git context)
- **THEN** the row's primary label is the basename of the worktree path

#### Scenario: Progress and modification time are shown

- **WHEN** an instance row is rendered for a change with at least one incomplete task
- **THEN** the row shows the instance's task progress as a fill meter (an outlined track with a fill whose width is `completedTasks / totalTasks`), with **no** inline digits
- **AND** the exact count is available via the meter's `title` tooltip ("N of M tasks") and its `role="progressbar"` aria attributes
- **AND** the row shows a relative modification time (e.g. `12m ago`)

### Requirement: Reactive Updates from Filesystem

The tree pane and the detail pane SHALL reflect on-disk changes within the watcher's debounce window without requiring user action. After the watcher finishes processing a debounced batch of filesystem events, the *first* refresh the frontend performs in response to that batch SHALL observe the post-batch state — the UI MUST NOT lag behind by one event for any on-disk change, including content-only changes inside a change directory that is already tracked (artifact file creation, task checkbox toggles, edits to spec or proposal markdown).

#### Scenario: Tree updates when new change appears

- **WHEN** a new change directory is created on disk in a registered workspace
- **THEN** the new change appears as a child of that workspace in the tree

#### Scenario: Detail pane updates when shown file is edited

- **WHEN** the detail pane is currently rendering an artifact's markdown
- **AND** that markdown file is modified on disk
- **THEN** the detail pane re-renders with the updated content

#### Scenario: Tree updates when change is archived on disk

- **WHEN** a change directory is moved from `openspec/changes/<id>/` to `openspec/changes/archive/<id>/`
- **THEN** the change is removed from the tree

#### Scenario: Artifact row flips to present when its file is created inside an existing change

- **WHEN** a change directory already exists and is tracked by the watcher (for example because `openspec new change` previously wrote only its `.openspec.yaml`)
- **AND** a subsequent on-disk write creates one of the four artifact files (`proposal.md`, `design.md`, `tasks.md`, or a `specs/<capability>/spec.md`) inside that change directory
- **THEN** the corresponding artifact row in the tree re-renders as present (full opacity, interactive) within the watcher's debounce window
- **AND** the row reaches its present state on the first refresh the frontend performs after that write — no further on-disk edit or user action is required to flip the row

#### Scenario: Instance-row task progress updates when a checkbox is toggled

- **WHEN** an instance row (or, for a singleton logical change, the flattened row) is rendered in the tree with a task-progress meter
- **AND** an on-disk edit to that change's `tasks.md` flips a task line's checkbox between `- [ ]` and `- [x]`
- **THEN** the row's task-progress meter re-renders with its fill width reflecting the new completion ratio within the watcher's debounce window
- **AND** the new fill is visible on the first refresh the frontend performs after that edit — no further edit, focus change, or window action is required to surface it

#### Scenario: Section completion glyph and auto-collapse update when the last task in a section is toggled

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an on-disk edit to `tasks.md` toggles the last incomplete task in that section from `- [ ]` to `- [x]`
- **THEN** the Section row's trailing `✓` glyph and the Section's auto-collapsed rendering both appear within the watcher's debounce window
- **AND** both are visible on the first refresh the frontend performs after that edit

### Requirement: Auto-Collapse of Completed Task Groups

The workspace tree SHALL auto-collapse two node types when their work is complete, so the user's attention is drawn to in-progress work:

- The **Tasks artifact node** of a change is considered complete when the change has at least one task (`totalTasks > 0`) and every task is complete (`completedTasks === totalTasks`).
- A **Section node** is considered complete when its section has at least one task and every task in it is complete.

When a node is complete, its default expansion state SHALL be "collapsed". When it is not complete (or has no tasks at all), its default expansion state SHALL be "expanded". The default is recomputed on every render from the node's current data, so transitions between in-progress and complete take effect within the watcher's debounce window with no extra user action.

The auto-collapse rule SHALL apply only to Tasks artifact nodes and Section nodes. Change rows, Instance rows, Repo groups, flat workspaces, multi-instance logical-change parents, the Proposal/Specs/Design artifact rows, capability rows under Specs, and individual task rows SHALL continue to default to "expanded" regardless of completion state.

A user override (a click on the disclosure caret) SHALL take precedence over the default and SHALL persist across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: Tasks artifact collapses when all tasks complete

- **WHEN** a change has at least one task and every task is complete
- **AND** the user has not explicitly expanded the Tasks artifact node since it became complete
- **THEN** the Tasks artifact node is rendered collapsed
- **AND** its meta slot shows the trailing `✓` completion glyph (its label no longer carries a textual count — see *Tasks Artifact Node Progress*)

#### Scenario: Section collapses when all its tasks complete

- **WHEN** a Section has at least one task and every task in it is complete
- **AND** the user has not explicitly expanded that Section since it became complete
- **THEN** the Section node is rendered collapsed

#### Scenario: Tasks artifact stays expanded when partially complete

- **WHEN** a change has at least one task and at least one of them is incomplete
- **THEN** the Tasks artifact node is rendered expanded by default

#### Scenario: Section stays expanded when partially complete

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section node is rendered expanded by default

#### Scenario: Section with no tasks is unaffected by the auto-collapse rule

- **WHEN** a Section has zero tasks
- **THEN** the Section row is rendered as a leaf (no chevron), as it is today
- **AND** the auto-collapse rule does not apply

#### Scenario: Tasks artifact with no tasks is unaffected by the auto-collapse rule

- **WHEN** a change's `tasks.md` exists but contains no parseable tasks (`totalTasks === 0`)
- **THEN** the Tasks artifact node renders per its existing behaviour (collapsible iff it has sections, defaulting to expanded)
- **AND** the auto-collapse rule does not apply

#### Scenario: Completing the last task in a Section auto-collapses it

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an external edit to `tasks.md` marks the last incomplete task complete
- **AND** the watcher emits the update
- **THEN** the Section node is re-rendered collapsed within the watcher debounce window
- **AND** the user does not need to take any action

#### Scenario: Adding an incomplete task to a complete Section re-expands it

- **WHEN** a Section node is rendered collapsed because every task in it is complete
- **AND** an external edit to `tasks.md` adds a new incomplete task to that section
- **AND** the watcher emits the update
- **THEN** the Section node is re-rendered expanded within the watcher debounce window

#### Scenario: User can expand an auto-collapsed group

- **WHEN** a Tasks artifact node or Section node is rendered collapsed because of the auto-collapse rule
- **AND** the user clicks the row's disclosure caret
- **THEN** the node is re-rendered expanded
- **AND** the expansion persists across restarts

### Requirement: Completed Section Row Shows a Completion Glyph

Every Section row whose section has at least one task and whose every task is complete SHALL display a ✓ glyph in the row's meta column, regardless of the row's current expansion state.

This glyph distinguishes a Section that is collapsed because all its tasks are done from a Section the user has manually collapsed while work is still in progress. It mirrors the trailing ✓ glyph rendered in the Change-row meta cluster when every task in a change is complete (see *Change-Row Completion Glyph*) and the trailing ✓ rendered on the Tasks artifact node at completion (see *Tasks Artifact Node Progress*).

#### Scenario: Completed Section shows the glyph while collapsed

- **WHEN** a Section is rendered collapsed because every task in it is complete
- **THEN** the Section row displays a ✓ glyph in its meta column

#### Scenario: Completed Section shows the glyph while expanded

- **WHEN** a Section is rendered expanded (either by default because it has incomplete tasks, or because the user explicitly expanded an auto-collapsed Section)
- **AND** every task in the Section is in fact complete
- **THEN** the Section row displays the ✓ glyph in its meta column

#### Scenario: In-progress Section shows no glyph

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section row does not display the ✓ glyph

#### Scenario: Empty Section shows no glyph

- **WHEN** a Section has zero tasks
- **THEN** the Section row does not display the ✓ glyph

### Requirement: Change-Row Completion Glyph

For change-aggregating rows that surface task progress — specifically the flat-workspace change row (`FlatChangeNode`) and the per-instance row (`InstanceNode`) — when every parsed task in the change is complete (`totalTasks > 0` and `completedTasks === totalTasks`), the row SHALL render a trailing `Check` glyph in the row's meta cluster. On the per-instance row, the in-progress task-progress meter is hidden at 100% (see *Instance Row Chrome* and the *Task Progress Meter* requirement in `visual-identity`) and the `Check` occupies the meta position the meter would otherwise hold. When at least one task is incomplete, or when the change has no tasks at all, the row SHALL NOT render the trailing `Check` glyph.

The `Check` glyph SHALL NOT appear in the row's leading slot on either row type. Pre-existing leading-position completion markers (specifically the leading `Check` on `FlatChangeNode` rendered when all tasks were done) SHALL be removed.

#### Scenario: Flat-change row gets a trailing tick when all tasks complete

- **WHEN** a flat-workspace change row is rendered for a change with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph
- **AND** no `Check` glyph appears in the row's leading slot

#### Scenario: Instance row gets a trailing tick when all tasks complete

- **WHEN** a per-instance change row is rendered for an instance with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph and renders no task-progress meter (the meter is hidden at 100%)
- **AND** the glyph sits where the meter would otherwise be, between the leading meta and the modification-time element

#### Scenario: Rows without complete tasks have no trailing tick

- **WHEN** a flat-change row or instance row is rendered for a change with at least one incomplete task, or for a change with no tasks at all
- **THEN** the row's meta cluster contains no `Check` glyph
- **AND** the leading slot also contains no `Check` glyph

## ADDED Requirements

### Requirement: Tasks Artifact Node Progress

The Tasks artifact node under a change SHALL render its label as the plain text `Tasks`, with no parenthetical `(completed/total)` count appended. The node's completion progress SHALL instead be surfaced in the node row's trailing meta slot:

- While the change has at least one task and not every task is complete (`totalTasks > 0` and `completedTasks < totalTasks`), the node SHALL render a task-progress meter (see the *Task Progress Meter* requirement in the `visual-identity` capability) in its meta slot.
- When every task is complete (`totalTasks > 0` and `completedTasks === totalTasks`), the node SHALL render a trailing `✓` glyph in its meta slot in place of the meter, mirroring the Change-row completion glyph.
- When the change has no parseable tasks (`totalTasks === 0`), the node SHALL render neither a meter nor a `✓` glyph in its meta slot.

This requirement does not alter the Tasks node's auto-collapse default (see *Auto-Collapse of Completed Task Groups*); it changes only the node's label and the contents of its meta slot.

#### Scenario: In-progress Tasks node shows a meter

- **WHEN** the Tasks artifact node is rendered for a change with at least one incomplete task
- **THEN** the node's label is the plain text `Tasks` (no `(n/n)` suffix)
- **AND** the node's meta slot shows a task-progress meter whose fill width is `completedTasks / totalTasks`
- **AND** the exact count is available via the meter's `title` tooltip and aria attributes

#### Scenario: Completed Tasks node shows a check instead of the meter

- **WHEN** the Tasks artifact node is rendered for a change in which every task is complete
- **THEN** the node's meta slot shows a trailing `✓` glyph and no meter

#### Scenario: Tasks node with no parseable tasks shows neither meter nor check

- **WHEN** the Tasks artifact node is rendered for a change whose `tasks.md` parses zero tasks (`totalTasks === 0`)
- **THEN** the node's label is the plain text `Tasks`
- **AND** the node's meta slot contains neither a meter nor a `✓` glyph
