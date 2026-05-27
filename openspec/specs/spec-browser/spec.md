# spec-browser Specification

## Purpose

Defines the master-detail browser surface of the desktop application that lets users navigate the OpenSpec artifacts of every registered workspace and read their rendered markdown content in a single window.
## Requirements
### Requirement: Master-Detail Layout

The main application window SHALL present a two-pane master-detail layout: a tree-navigation pane on the left and a content-rendering pane on the right. A resizable divider separates the two panes.

#### Scenario: Both panes visible at startup

- **WHEN** the user opens the main window for the first time
- **THEN** both the tree pane and the detail pane are visible side by side
- **AND** the divider between them can be dragged to adjust pane widths

### Requirement: Workspace Tree Hierarchy

The tree pane SHALL display tracked workspaces grouped by repository where applicable. For each git repository with at least one tracked workspace, the tree SHALL render a top-level Repo group node containing the repository's logical changes. For each non-git workspace, the tree SHALL render a top-level workspace node containing that workspace's changes directly, as before.

A logical change groups every `ChangeInstance` that shares the same `(repository_id, change_directory_name)` tuple. Inside a Repo group, each logical change is rendered according to its instance count: a logical change with exactly one instance SHALL be rendered as a flat instance row with no parent disclosure; a logical change with two or more instances SHALL be rendered as a disclosure parent row with one child row per instance.

Each `ChangeInstance` row, when rendered, SHALL expose the same four artifact nodes — Proposal, Specs, Design, Tasks — in fixed order, mirroring the existing artifact subtree. The Specs node, when present, contains one child per capability spec file. The Tasks node, when present, contains one child per section in that instance's `tasks.md`, and each section contains one child per task line.

A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — without changing the detail pane (consistent with the existing `repo` / `workspace` selection contract for grouping nodes). No placeholder child row SHALL be rendered beneath an empty top-level row.

#### Scenario: Git repo with multiple worktrees shown as one Repo group

- **WHEN** a repository has three tracked worktrees, two of which contain a change with the same directory name
- **THEN** the tree shows one top-level Repo group for that repository
- **AND** the two-instance change appears under a disclosure parent row with both instances as children
- **AND** any single-instance change appears as a flat row directly under the Repo group

#### Scenario: Non-git workspace shown as a standalone top-level node

- **WHEN** a tracked workspace is not inside a git repository
- **THEN** the workspace is rendered as a top-level node (not a Repo group)
- **AND** the workspace's changes are rendered directly underneath without instance aggregation

#### Scenario: Artifact subtree appears under each instance

- **WHEN** an instance row is expanded (or, for a singleton, the flattened row is expanded)
- **THEN** the four artifact nodes appear in the order: Proposal, Specs, Design, Tasks
- **AND** the contents of each artifact node are read from that instance's `worktree_path`

#### Scenario: Archived logical changes are not shown in the Active section

- **WHEN** every instance of a logical change is under `openspec/changes/archive/` in its worktree
- **THEN** the logical change is not shown in the Active section of the tree
- **AND** it appears in the Archive section instead (if rendered) or is hidden, matching the existing archived-change behaviour

#### Scenario: Empty top-level row renders as a leaf

- **WHEN** a top-level Repo group node or non-git workspace node has zero active changes
- **THEN** the row renders as a leaf with no disclosure chevron and no toggle affordance
- **AND** the row's count badge displays `0`
- **AND** clicking the row updates the tree's selected-node state and applies the same visual selection treatment a non-empty top-level row receives
- **AND** the detail pane is not changed by the click (consistent with the existing selection contract for `repo` and `workspace` grouping nodes)
- **AND** no placeholder child row (such as "no active changes") is rendered beneath the row

#### Scenario: Empty top-level row becomes non-empty when a change is added

- **WHEN** a top-level row was rendering as a leaf because it had zero active changes
- **AND** the watcher reports a new active change for that workspace
- **THEN** the row re-renders as a disclosure parent
- **AND** the count badge advances from `0` to the new count
- **AND** the disclosure's open/closed state is governed by the user's persisted override for that row, if any, and otherwise by the row's default-open behaviour

### Requirement: Top-Level Row Display Name and Tint

The tree pane SHALL render every top-level row — a flat workspace node or a repository group node — using the row's configured display name when one is set, and using the row's derived default name (the folder basename for a flat workspace, the main worktree's basename for a repository group) when none is set. The tree pane SHALL render the row's background with the tint corresponding to the row's configured palette colour when one is set, and with the default row background when none is set.

The tint SHALL be applied to the top-level row only. Child rows (logical changes, instances, artifact nodes, sections, tasks, capability spec rows) SHALL NOT inherit the tint and SHALL continue to render with the default row background. The tint MUST compose cleanly with the existing selection highlight so a selected top-level row remains visually distinct from its unselected neighbours.

When the row's configured palette colour is absent (either because no presentation entry exists, or because the user has explicitly chosen "none"), the row SHALL render with no tint, identical to today's behaviour for that row.

#### Scenario: Top-level row uses configured display name

- **WHEN** a flat workspace has a configured display name
- **THEN** its top-level tree row renders with that display name
- **AND** the configured name is also used wherever the row is referenced (for example, the row's accessible label)

#### Scenario: Top-level row falls back to derived name when no display name is configured

- **WHEN** a flat workspace or a repository group has no configured display name
- **THEN** its top-level tree row renders with the folder basename (or main worktree basename, for a repository group)

#### Scenario: Top-level row is tinted with the configured palette colour

- **WHEN** a flat workspace or a repository group has a configured palette colour
- **THEN** its top-level tree row background renders with the tint corresponding to that colour token
- **AND** child rows below it render with the default row background, not the tint

#### Scenario: Top-level row is untinted when no palette colour is configured

- **WHEN** a flat workspace or a repository group has no configured palette colour
- **THEN** its top-level tree row background renders with the default row background, indistinguishable from the same row before the presentation store was introduced

#### Scenario: Selection highlight remains visible over the tint

- **WHEN** the user selects a tinted top-level row
- **THEN** the row's selected state remains visually distinct from its unselected appearance
- **AND** the configured tint is still discernible underneath the selection treatment

#### Scenario: Presentation update re-renders the row without a manual refresh

- **WHEN** the user changes the display name or palette colour of a workspace from the Settings view
- **THEN** the corresponding top-level row in the tree pane updates to reflect the new name and tint without the user having to close and reopen the window or otherwise force a refresh

### Requirement: Markdown Rendering of Leaf Artifacts

Clicking a leaf artifact node (Proposal, Design, Tasks, or an individual capability spec under the Specs node) SHALL render that artifact's markdown file in the detail pane. Rendering MUST support GitHub-Flavored Markdown including syntax-highlighted fenced code blocks.

#### Scenario: Click proposal renders proposal.md

- **WHEN** the user clicks a change's Proposal node
- **THEN** the detail pane shows the rendered content of `proposal.md` for that change

#### Scenario: Click design renders design.md

- **WHEN** the user clicks a change's Design node
- **THEN** the detail pane shows the rendered content of `design.md` for that change

#### Scenario: Click tasks renders tasks.md

- **WHEN** the user clicks a change's Tasks node
- **THEN** the detail pane shows the rendered content of `tasks.md` for that change

#### Scenario: Click individual capability spec renders that spec.md

- **WHEN** the user clicks a child node under the Specs artifact node
- **THEN** the detail pane shows the rendered content of `specs/<capability>/spec.md` for that change

### Requirement: Section and Task Scroll Anchors

Clicking a section or individual-task node SHALL render `tasks.md` in the detail pane (if not already rendered) and scroll the detail pane to the corresponding heading or line.

#### Scenario: Click section scrolls to heading

- **WHEN** the user clicks a section node under a Tasks artifact
- **THEN** the detail pane shows the rendered `tasks.md` for that change
- **AND** the pane is scrolled so the section's heading is visible at the top of the pane

#### Scenario: Click task scrolls to task line

- **WHEN** the user clicks an individual task node under a section
- **THEN** the detail pane shows the rendered `tasks.md` for that change
- **AND** the pane is scrolled so the task's line is visible

### Requirement: Deferred Interaction Nodes

In v1, clicking a top-level workspace node, a Repo group node, a logical-change parent disclosure row, a change node, or the Specs artifact node SHALL produce no observable effect in the detail pane. These node types are reserved for later UX work or are pure disclosure rows by design.

#### Scenario: Click Repo group is a no-op

- **WHEN** the user clicks a Repo group node
- **THEN** the detail pane's current contents are unchanged

#### Scenario: Click logical-change parent disclosure is a no-op

- **WHEN** the user clicks a logical-change parent disclosure row of a multi-instance change
- **THEN** the detail pane's current contents are unchanged
- **AND** the row's expand/collapse state toggles in response to the click on its disclosure caret

#### Scenario: Click workspace is a no-op

- **WHEN** the user clicks a top-level workspace node (for a non-git workspace)
- **THEN** the detail pane's current contents are unchanged

#### Scenario: Click change is a no-op

- **WHEN** the user clicks a change node under a non-git workspace
- **THEN** the detail pane's current contents are unchanged

#### Scenario: Click Specs artifact node is a no-op

- **WHEN** the user clicks the Specs artifact node of an instance or a change
- **THEN** the detail pane's current contents are unchanged

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

- **WHEN** an instance row (or, for a singleton logical change, the flattened row) is rendered in the tree with a task-progress count
- **AND** an on-disk edit to that change's `tasks.md` flips a task line's checkbox between `- [ ]` and `- [x]`
- **THEN** the row's `(completed/total)` progress label re-renders with the new completion count within the watcher's debounce window
- **AND** the new count is visible on the first refresh the frontend performs after that edit — no further edit, focus change, or window action is required to surface it

#### Scenario: Section completion glyph and auto-collapse update when the last task in a section is toggled

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an on-disk edit to `tasks.md` toggles the last incomplete task in that section from `- [ ]` to `- [x]`
- **THEN** the Section row's trailing `✓` glyph and the Section's auto-collapsed rendering both appear within the watcher's debounce window
- **AND** both are visible on the first refresh the frontend performs after that edit

### Requirement: Window State Persistence

The main window's position, size, and maximised state SHALL persist across application restarts.

#### Scenario: Window size restored after relaunch

- **WHEN** the user resizes the main window to a non-default dimension
- **AND** quits and relaunches the application
- **THEN** the main window opens at the previously set size and position

### Requirement: Read-Only Viewer

In v1, the application SHALL NOT modify any spec file as a result of user interaction with the rendered content.

#### Scenario: Task checkboxes are not interactive

- **WHEN** the detail pane is rendering a `tasks.md` containing markdown task checkboxes
- **THEN** clicking a rendered checkbox does not modify the underlying file

### Requirement: Active-Instance Indicator

For every logical change with at least two instances, the application SHALL identify the *primary instance* as the one with the most recent modification time across the files of its change directory, and render a visible active indicator (●) on that instance's row. Singleton logical changes (one instance) SHALL NOT display the active indicator — there is nothing to disambiguate.

#### Scenario: Most-recently-modified instance carries the active dot

- **WHEN** a logical change has two instances and one has been modified more recently than the other
- **THEN** the more recently modified instance's row displays the ● indicator
- **AND** the other instance's row does not display the indicator

#### Scenario: Indicator moves when activity moves

- **WHEN** the secondary instance of a logical change is modified, making it the more recently modified one
- **THEN** the ● indicator moves to that instance's row within the watcher debounce window

### Requirement: Per-Instance Divergence Label

For every `ChangeInstance` that is not on the repository's default branch, the application SHALL compute and display at most one divergence label by comparing the instance's change directory contents against the default-branch instance of the same logical change. The labels are:

- `[diverged]` — the change exists in both the default-branch instance and the non-default instance, but the file contents differ at the byte level.
- `[stale]` — the change is archived on the default branch (under `openspec/changes/archive/`) but is still active in the non-default instance.

If the change does not exist on the default branch at all, or if no default branch is known, or if the contents are identical, the instance SHALL display no divergence label.

#### Scenario: Diverged content gets the diverged label

- **WHEN** an instance on a non-default branch has different content under `openspec/changes/<name>/` than the default-branch instance of the same logical change
- **THEN** the instance row displays the `[diverged]` label

#### Scenario: Stale-vs-archive gets the stale label

- **WHEN** the default-branch instance of a logical change is in `openspec/changes/archive/<name>/`
- **AND** a non-default instance of the same logical change is in `openspec/changes/<name>/` (still active)
- **THEN** the non-default instance row displays the `[stale]` label

#### Scenario: Branch-only change gets no label

- **WHEN** a logical change has no instance on the default branch (it was created only on a feature branch)
- **THEN** every non-default instance displays no divergence label

#### Scenario: Identical content gets no label

- **WHEN** a non-default instance has byte-identical content to the default-branch instance of the same logical change
- **THEN** the non-default instance row displays no divergence label

#### Scenario: No default branch produces no labels

- **WHEN** the repository has no detected default branch
- **THEN** no instance of any logical change in that repository displays a divergence label

### Requirement: Singleton Logical-Change Flattening and Promotion

A logical change with exactly one instance SHALL be rendered as a single flat row directly under its Repo group (or under the Active / Archive section as appropriate). When the instance count grows to two or more — for example because a new worktree begins working on the same change — the row SHALL be promoted to a disclosure parent with one child row per instance. When the instance count drops back to one, the row SHALL collapse back to a flat row.

#### Scenario: Singleton renders without a disclosure parent

- **WHEN** a logical change has exactly one instance
- **THEN** the tree shows a single row for that instance directly under its Repo group
- **AND** no separate parent disclosure row is rendered

#### Scenario: Promotion when a second instance appears

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the row is replaced with a disclosure parent row that, when expanded, shows both instances
- **AND** the parent is expanded by default so the user sees both new and previously-visible instances without an extra click

#### Scenario: Collapse when count drops back to one

- **WHEN** a previously multi-instance logical change loses every instance but one
- **THEN** the disclosure parent disappears and the remaining instance is rendered as a flat row

### Requirement: Instance Row Chrome

Each `ChangeInstance` row SHALL display the instance's branch name as its primary label, falling back to the worktree path's basename when the branch is not known (detached HEAD, bare worktree, non-git workspace re-using this rendering path). The row SHALL additionally display the task-progress count of the instance and the relative modification time. Divergence labels (when present) and the active indicator (when present) attach to the row alongside these elements.

#### Scenario: Instance label uses branch when available

- **WHEN** an instance's worktree is on a named branch
- **THEN** the row's primary label is the branch name

#### Scenario: Instance label falls back to path basename

- **WHEN** an instance's worktree is not on a named branch (detached HEAD or no git context)
- **THEN** the row's primary label is the basename of the worktree path

#### Scenario: Progress and modification time are shown

- **WHEN** an instance row is rendered
- **THEN** the row shows the instance's task progress (e.g. `3/8`)
- **AND** the row shows a relative modification time (e.g. `12m ago`)

### Requirement: Default Expansion of Tree Nodes

Every collapsible node in the workspace tree SHALL be rendered with a default expansion state derived from the node's current data at render time, with no prior interaction required.

For most node types — Repo groups, flat workspaces, multi-instance logical-change parents, instance rows, the Proposal/Design/Specs artifact nodes, capability rows under Specs, and individual task rows — the default SHALL be "expanded".

For two node types the default depends on completion state:

- The **Tasks artifact node** SHALL default to "collapsed" when its change has at least one task and every task is complete; otherwise it SHALL default to "expanded".
- A **Section node** SHALL default to "collapsed" when its section has at least one task and every task in it is complete; otherwise it SHALL default to "expanded".

The user MAY override a node's default in either direction by clicking its disclosure caret. The application records overrides in two independent sets:

- A `collapsed` set of node IDs the user has closed against a default-open node.
- An `expanded` set of node IDs the user has opened against a default-closed node.

A node's rendered open/closed state SHALL be computed as follows:

- For a node whose current default is "open": open iff its ID is **not** in the `collapsed` set.
- For a node whose current default is "closed": open iff its ID **is** in the `expanded` set.

#### Scenario: First-ever launch shows the tree expanded except for completed groups

- **WHEN** the user launches the application for the first time after this change ships
- **AND** at least one workspace has been registered
- **THEN** every collapsible row whose computed default is "expanded" is rendered open
- **AND** every Tasks artifact node whose change has at least one task and all tasks complete is rendered collapsed
- **AND** every Section node whose section has at least one task and all of them complete is rendered collapsed
- **AND** every section with at least one incomplete task is rendered expanded along with its task rows

#### Scenario: New change appears with completion-aware default expansion

- **WHEN** the workspace tree is already rendered
- **AND** a new change directory is added to a registered workspace on disk, triggering a `change-added` event
- **THEN** the new change's row appears in the tree expanded
- **AND** the change's Proposal, Specs, and Design artifact rows are rendered expanded
- **AND** the Tasks artifact row is rendered collapsed iff the change's tasks are all complete, otherwise expanded
- **AND** each Section row under Tasks is rendered per the same per-node-type default rule

#### Scenario: Promoted multi-instance parent appears expanded

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the new disclosure parent row is rendered expanded by default
- **AND** the two instance rows beneath it are visible without any user click

#### Scenario: User expand overrides an auto-collapsed Section

- **WHEN** a Section node is rendered collapsed because every task in it is complete
- **AND** the user clicks the Section row's disclosure caret
- **THEN** the Section is re-rendered expanded
- **AND** its task rows are visible

#### Scenario: User collapse overrides a default-open node

- **WHEN** an in-progress Section node is rendered expanded
- **AND** the user clicks its disclosure caret
- **THEN** the Section is re-rendered collapsed
- **AND** its row continues to display its title (no other rows beneath it change state)

### Requirement: User Collapse State Persists Across Sessions

When the user clicks the disclosure caret of any tree node, the application SHALL persist the resulting override so that, after quitting and relaunching, the node is rendered in the same state without further user action.

The persisted state SHALL consist of two independent sets of node IDs:

- `collapsedTreeNodeIds` — node IDs the user has closed against a default-open node.
- `expandedTreeNodeIds` — node IDs the user has opened against a default-closed node.

A node's rendered state on launch SHALL be computed by combining its current default (derived from the node's data) with the matching override set, as defined in the *Default Expansion of Tree Nodes* requirement.

A persisted ID in one set whose node's default has since flipped to the other polarity SHALL be ignored — it is consulted only when the node's default again matches that set's role. The application is not required to garbage-collect such inert entries.

#### Scenario: Collapsed default-open node stays collapsed after restart

- **WHEN** the user collapses a default-open node (e.g., the Proposal artifact row of some change, or an in-progress Section)
- **AND** the user quits and relaunches the application
- **THEN** the same node is rendered in its collapsed state in the restored tree
- **AND** sibling nodes the user did not collapse are rendered per their own defaults

#### Scenario: Re-expanded default-open node stays expanded after restart

- **WHEN** the user has previously collapsed and persisted a default-open node
- **AND** the user re-expands that node in the current session
- **AND** the user quits and relaunches the application
- **THEN** the node is rendered in its expanded state in the restored tree

#### Scenario: Expanded default-closed node stays expanded after restart

- **WHEN** a Section node is collapsed by default because every task in it is complete
- **AND** the user clicks its disclosure caret to expand it
- **AND** the user quits and relaunches the application
- **THEN** the same Section is rendered in its expanded state in the restored tree
- **AND** other completed Sections the user did not expand remain collapsed

#### Scenario: Re-collapsed default-closed node stays collapsed after restart

- **WHEN** the user has previously expanded and persisted a default-closed Section
- **AND** the user re-collapses that Section in the current session
- **AND** the user quits and relaunches the application
- **THEN** the Section is rendered in its collapsed state in the restored tree

#### Scenario: Settings file with no expanded-IDs field loads cleanly

- **WHEN** the user launches a version of the application that supports the expanded-overrides set for the first time
- **AND** the existing settings file on disk was written by a previous version that has no `expandedTreeNodeIds` field
- **THEN** the application loads the settings file successfully
- **AND** the existing `collapsedTreeNodeIds` field is honoured as before
- **AND** the tree is rendered with an empty `expanded` override set, so every default-closed node renders collapsed until the user expands it

#### Scenario: Persistence write is bounded by user toggles

- **WHEN** the user toggles the same node open and closed several times in rapid succession
- **THEN** the persisted state eventually reflects the final toggled position for both sets
- **AND** the application does not write a settings file for every intermediate state (writes to each set are coalesced)

#### Scenario: Stale expand-override survives default flip without surfacing

- **WHEN** the user has expanded a completed Section (its ID is in the `expanded` set)
- **AND** a new incomplete task is added to that Section, flipping its default to "open"
- **THEN** the Section is rendered open (because the default-open path consults the `collapsed` set, which does not contain the ID)
- **AND** when the Section later returns to fully-complete, the persisted ID in the `expanded` set causes the Section to render expanded again (matching the user's earlier preference)

### Requirement: Tree Expansion Has No First-Sight Auto-Expansion Effect

The application SHALL NOT maintain a separate "first time we see this node, mark it expanded (or collapsed)" code path. A node's default state is derived from its current data on every render — not from any one-shot seeding effect that runs on view changes — and the user's override (if any) is applied on top of that default.

#### Scenario: No second-mount re-seeding

- **WHEN** the watcher emits a `cache-updated` event that causes the tree's `views` prop to re-render
- **THEN** the application does not run any effect that mutates the `collapsed` or `expanded` override sets in response to the new view

#### Scenario: User override survives a tree re-render

- **WHEN** the user has expanded an auto-collapsed Section or collapsed a default-open Section
- **AND** the watcher subsequently emits a `cache-updated` event for that workspace
- **THEN** the user's override is preserved after the re-render
- **AND** the application does not flip the node's state as a side effect of the view change

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
- **AND** its `(n/n)` label still indicates the completion count

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

This glyph distinguishes a Section that is collapsed because all its tasks are done from a Section the user has manually collapsed while work is still in progress. It mirrors the trailing ✓ glyph rendered in the Change-row meta cluster when every task in a change is complete (see *Change-Row Completion Glyph*), and complements the textual `(n/n)` label on the Tasks artifact node.

The Tasks artifact node SHALL NOT receive an additional glyph; its existing `Tasks (n/n)` label already conveys completion textually.

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

### Requirement: Artifact Row Presence Treatment

For each artifact node (Proposal, Specs, Design, Tasks) rendered under an instance row or a flat-change row, the row's *leading slot* (the position to the immediate right of the chevron/spacer) SHALL be reserved for identity affordances only — the row SHALL NOT render an icon whose sole semantics are "the underlying artifact file is present on disk." When the artifact's underlying file is present, the row SHALL display only the chevron (or chevron-spacer), the row label, and any trailing meta the schema defines.

When the artifact's underlying file is absent, the row SHALL:

- render at `opacity: 0.45` of the row's normal appearance,
- set `pointer-events: none` (or otherwise be inert to mouse interaction) so that clicking the row produces no selection, no detail-pane change, and no hover styling,
- preserve its layout footprint (chevron-spacer, label, depth indent) so the four-row artifact block does not collapse,
- continue to be visible in the tree as a slot indicator for the missing artifact.

The Specs artifact node SHALL count as "present" iff at least one capability spec file is parsed under the change; otherwise it SHALL be treated as absent and dimmed per the rule above.

#### Scenario: Present artifact rows carry no leading existence icon

- **WHEN** an artifact node is rendered for an artifact whose underlying file is present
- **THEN** the row displays no leading existence-marker glyph (no `Check`, no `DotOutline`, no equivalent)
- **AND** the row renders at full opacity
- **AND** the row participates normally in click, hover, and selection

#### Scenario: Missing artifact rows are dimmed and non-interactive

- **WHEN** an artifact node is rendered for an artifact whose underlying file is absent
- **THEN** the row renders at `opacity: 0.45`
- **AND** the row does not respond to clicks (no selection, no detail-pane change)
- **AND** the row does not display a hover background
- **AND** the row still occupies its full layout slot (label visible, depth indent preserved) so the four-artifact block remains intact

#### Scenario: Specs artifact dimming follows capability-spec presence

- **WHEN** a change has no parsed capability spec files
- **THEN** the Specs artifact row is treated as absent and rendered dim + non-interactive
- **AND** when at least one capability spec file is parsed, the Specs row renders normally

### Requirement: Change-Row Completion Glyph

For change-aggregating rows that surface a task progress count — specifically the flat-workspace change row (`FlatChangeNode`) and the per-instance row (`InstanceNode`) — when every parsed task in the change is complete (`totalTasks > 0` and `completedTasks === totalTasks`), the row SHALL render a trailing `Check` glyph in the row's meta cluster, alongside the progress count. The glyph SHALL appear adjacent to the progress count (between progress and any modification-time element). When at least one task is incomplete, or when the change has no tasks at all, the row SHALL NOT render the trailing `Check` glyph.

The `Check` glyph SHALL NOT appear in the row's leading slot on either row type. Pre-existing leading-position completion markers (specifically the leading `Check` on `FlatChangeNode` rendered when all tasks were done) SHALL be removed.

#### Scenario: Flat-change row gets a trailing tick when all tasks complete

- **WHEN** a flat-workspace change row is rendered for a change with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph alongside the progress count
- **AND** no `Check` glyph appears in the row's leading slot

#### Scenario: Instance row gets a trailing tick when all tasks complete

- **WHEN** a per-instance change row is rendered for an instance with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph alongside the progress count
- **AND** the glyph sits adjacent to the progress count, between progress and the modification-time element

#### Scenario: Rows without complete tasks have no trailing tick

- **WHEN** a flat-change row or instance row is rendered for a change with at least one incomplete task, or for a change with no tasks at all
- **THEN** the row's meta cluster contains no `Check` glyph
- **AND** the leading slot also contains no `Check` glyph

