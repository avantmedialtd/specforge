# spec-browser Specification

## Purpose

Defines the master-detail browser surface of the desktop application that lets users navigate the OpenSpec artifacts of every registered workspace and read their rendered markdown content in a single window.
## Requirements
### Requirement: Master-Detail Layout

The main application window SHALL present a master-detail layout of two primary panes — a tree-navigation pane on the left and a content-rendering (detail) pane in the center — plus an optional commit-graph rail on the far right (see the *Commit-Graph Rail Pane* requirement in the `commit-graph` capability). Resizable dividers separate the panes.

The detail (center) pane SHALL render one of four targets: an OpenSpec artifact's markdown, a commit's detail view when a commit is selected in the rail, the **Dashboard** (see the *Dashboard Home Surface* requirement in the `dashboard` capability), or the **Archive view** (see the *Archive View* requirement in the `archive-browser` capability) when the Archive entrypoint is active. The Dashboard SHALL be the default target: it is rendered at startup and whenever no artifact and no commit is selected and the Archive view is not open, in place of any "nothing selected" placeholder. The Archive view and the Settings view are modal pane targets toggled from their sidebar entrypoints; while either is open it takes precedence over the artifact/commit/Dashboard target, and closing it returns the pane to whichever of those was selected most recently. The tree drives the artifact target and the rail drives the commit target.

#### Scenario: Panes visible at startup

- **WHEN** the user opens the main window for the first time
- **THEN** the tree pane and the detail pane are visible side by side
- **AND** the commit-graph rail is visible on the far right
- **AND** the detail pane renders the Dashboard (no artifact or commit having been selected, and the Archive view not open)
- **AND** the dividers between the panes can be dragged to adjust their widths

#### Scenario: Detail pane renders the Dashboard by default

- **WHEN** no artifact and no commit is selected and the Archive view is not open
- **THEN** the detail pane renders the Dashboard
- **AND** no "nothing selected" placeholder is shown

#### Scenario: Detail pane renders artifact markdown by default

- **WHEN** the user selects a renderable artifact node in the tree
- **THEN** the detail pane renders that artifact's markdown

#### Scenario: Detail pane renders commit detail when a commit is selected

- **WHEN** the user selects a commit in the commit-graph rail
- **THEN** the detail pane renders that commit's detail view
- **AND** selecting an artifact node in the tree afterwards returns the detail pane to artifact markdown

#### Scenario: Detail pane renders the Archive view when its entrypoint is active

- **WHEN** the user activates the Archive entrypoint
- **THEN** the detail pane renders the Archive view in place of the artifact/commit/Dashboard target
- **AND** closing the Archive view returns the detail pane to the most recently selected artifact, commit, or the Dashboard

### Requirement: Workspace Tree Hierarchy

The tree pane SHALL display tracked workspaces grouped by repository where applicable. For each git repository with at least one tracked workspace, the tree SHALL render a top-level Repo group node containing the repository's logical changes. For each non-git workspace, the tree SHALL render a top-level workspace node containing that workspace's changes directly, as before.

A logical change groups every `ChangeInstance` that shares the same `(repository_id, change_directory_name)` tuple. Inside a Repo group, each logical change is rendered according to its instance count: a logical change with exactly one instance SHALL be rendered as a flat instance row with no parent disclosure; a logical change with two or more instances SHALL be rendered as a disclosure parent row with one child row per instance.

Each `ChangeInstance` row, when rendered, SHALL expose the same four artifact nodes — Proposal, Specs, Design, Tasks — in fixed order, mirroring the existing artifact subtree. The Specs node, when present, contains one child per capability spec file. The Tasks node, when present, contains one child per section in that instance's `tasks.md`, and each section contains one child per task line.

The tree SHALL render **active changes only**. Archived logical changes SHALL NOT appear anywhere in the tree — neither in an Active section nor in a separate Archive section. Archived changes are browsed exclusively in the dedicated Archive view (see the *Archive View* requirement in the `archive-browser` capability).

A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — and opens the workspace file browser for the row's workspace in the detail pane (see the `workspace-file-browser` capability). No placeholder child row SHALL be rendered beneath an empty top-level row. A top-level row whose only changes are archived SHALL therefore render as a leaf with a `0` active count, the same as a row with no changes at all.

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

#### Scenario: Archived logical changes are not shown in the tree

- **WHEN** every instance of a logical change is under `openspec/changes/archive/` in its worktree
- **THEN** the logical change is not shown anywhere in the tree
- **AND** it is browsable in the Archive view instead

#### Scenario: Empty top-level row renders as a leaf

- **WHEN** a top-level Repo group node or non-git workspace node has zero active changes
- **THEN** the row renders as a leaf with no disclosure chevron and no toggle affordance
- **AND** the row's count badge displays `0`
- **AND** clicking the row updates the tree's selected-node state and applies the same visual selection treatment a non-empty top-level row receives
- **AND** the detail pane shows the workspace file browser for the row's workspace (see the `workspace-file-browser` capability)
- **AND** no placeholder child row (such as "no active changes") is rendered beneath the row

#### Scenario: Top-level row with only archived changes renders as a leaf

- **WHEN** a top-level row has zero active changes but one or more archived changes
- **THEN** the row renders as a leaf with a `0` active count
- **AND** no archived change is rendered beneath it in the tree

#### Scenario: Empty top-level row becomes non-empty when a change is added

- **WHEN** a top-level row was rendering as a leaf because it had zero active changes
- **AND** the watcher reports a new active change for that workspace
- **THEN** the row re-renders as a disclosure parent
- **AND** the count badge advances from `0` to the new count
- **AND** the disclosure's open/closed state is governed by the user's persisted override for that row, if any, and otherwise by the row's default-open behaviour

### Requirement: Top-Level Row Display Name and Swatch

The tree pane SHALL render every top-level row — a flat workspace node or a repository group node — using the row's configured display name when one is set, and using the row's derived default name (the folder basename for a flat workspace, the main worktree's basename for a repository group) when none is set. The tree pane SHALL render an 8px filled circular swatch glyph between the row's chevron and its label, in the colour corresponding to the row's configured palette colour, when one is set. When no palette colour is configured the swatch SHALL be omitted.

The swatch SHALL be applied to the top-level row only. Child rows (logical changes, instances, artifact nodes, sections, tasks, capability spec rows) SHALL NOT render a swatch. The row's background SHALL be the default row background regardless of palette colour. The existing selection treatment (a 2px `--accent` `border-left`) SHALL compose with the swatch without modification: the swatch sits in the row's content area, the selection bar lives in the inline-start border slot, and the two signals do not overlap.

#### Scenario: Top-level row uses configured display name

- **WHEN** a flat workspace has a configured display name
- **THEN** its top-level tree row renders with that display name
- **AND** the configured name is also used wherever the row is referenced (for example, the row's accessible label)

#### Scenario: Top-level row falls back to derived name when no display name is configured

- **WHEN** a flat workspace or a repository group has no configured display name
- **THEN** its top-level tree row renders with the folder basename (or main worktree basename, for a repository group)

#### Scenario: Top-level row shows the configured palette colour as a swatch

- **WHEN** a flat workspace or a repository group has a configured palette colour
- **THEN** its top-level tree row renders an 8px filled circular swatch between the chevron and the label, in the colour corresponding to that palette token
- **AND** the row background is the default row background, unchanged by the palette colour
- **AND** child rows below it render no swatch and the default row background

#### Scenario: Top-level row omits the swatch when no palette colour is configured

- **WHEN** a flat workspace or a repository group has no configured palette colour
- **THEN** its top-level tree row renders no swatch
- **AND** the row background is the default row background, indistinguishable from the same row before the presentation store was introduced

#### Scenario: Selection highlight composes with the swatch

- **WHEN** the user selects a top-level row that has a configured palette colour
- **THEN** the row renders both the 2px `--accent` left border bar and the 8px swatch
- **AND** the two signals do not overlap visually (the bar is in the inline-start border slot; the swatch is in the row's content area)
- **AND** the row background is unchanged by the selected state

#### Scenario: Presentation update re-renders the row without a manual refresh

- **WHEN** the user changes the display name or palette colour of a workspace from the Settings view
- **THEN** the corresponding top-level row in the tree pane updates to reflect the new name and swatch without the user having to close and reopen the window or otherwise force a refresh

### Requirement: Inter-Workspace Divider

Successive top-level rows in the tree pane SHALL be separated by a 1px `var(--border)` horizontal hairline. The hairline SHALL be rendered as a `border-top` on every top-level row except the first, so that the first top-level row carries no top border and every subsequent top-level row carries one. The hairline replaces the section-header affordance previously provided by the full-row background tint.

The hairline SHALL apply only to top-level rows (flat workspace nodes and repository group nodes). Child rows SHALL NOT render a `border-top`. The hairline SHALL compose with the row's other visual signals — the swatch in the content area, the selection bar in the inline-start border slot, and any hover/focus state — without modification: it is a cross-axis 1px line and does not occupy the inline-start border slot.

#### Scenario: Second and subsequent workspaces render a hairline

- **WHEN** the tree pane renders two or more top-level rows
- **THEN** the second and every subsequent top-level row resolves a 1px `var(--border)` `border-top`
- **AND** the first top-level row resolves a `border-top` of `0`

#### Scenario: Child rows render no hairline

- **WHEN** a top-level row is expanded
- **THEN** none of its child rows (changes, instances, artifacts, sections, tasks, capability specs) renders a `border-top`
- **AND** the only horizontal separation between successive child rows is the row's vertical padding

#### Scenario: Hairline composes with selection and swatch on the same row

- **WHEN** the user selects a top-level row that is not the first top-level row and that has a configured palette colour
- **THEN** the row simultaneously renders the 1px `var(--border)` `border-top`, the 2px `--accent` `border-left`, and the 8px swatch in the content area
- **AND** no signal visually displaces or hides any other

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

Clicking a logical-change parent disclosure row, a change node, or the Specs artifact node SHALL produce no observable effect in the detail pane. These node types are pure disclosure rows by design or are reserved for later UX work. Clicking a top-level workspace node or a Repo group node is no longer deferred: it opens the workspace file browser in the detail pane — see the *File Browser Surface* requirement in the `workspace-file-browser` capability.

#### Scenario: Click logical-change parent disclosure is a no-op

- **WHEN** the user clicks a logical-change parent disclosure row of a multi-instance change
- **THEN** the detail pane's current contents are unchanged
- **AND** the row's expand/collapse state toggles in response to the click on its disclosure caret

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

- **WHEN** an instance row (or, for a singleton logical change, the flattened row) is rendered in the tree with a task-progress meter
- **AND** an on-disk edit to that change's `tasks.md` flips a task line's checkbox between `- [ ]` and `- [x]`
- **THEN** the row's task-progress meter re-renders with its fill width reflecting the new completion ratio within the watcher's debounce window
- **AND** the new fill is visible on the first refresh the frontend performs after that edit — no further edit, focus change, or window action is required to surface it

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

A **multi-instance child row** — a `ChangeInstance` rendered beneath a multi-instance logical-change disclosure parent — SHALL display the instance's branch name as its primary label, falling back to the worktree path's basename when the branch is not known (detached HEAD, bare worktree). Because the disclosure parent already names the change, the branch alone distinguishes the child, and the child SHALL remain a single line. The child row SHALL additionally display, in its trailing meta slot, a task-progress meter (see the *Task Progress Meter* requirement in the `visual-identity` capability) and the relative modification time. The divergence label (when present) and the active indicator (when present) attach to the child row alongside these elements.

A **flattened singleton instance row** (a logical change with exactly one instance, per *Singleton Logical-Change Flattening and Promotion*) and a **flat-workspace change row** are NOT governed by this requirement. They render per *Two-Line Sole-Change-Row Layout*, which gives the change's own label the primary line and relocates the branch, worktree folder, and status meta to a second line. In particular, branch-as-primary-label is a multi-instance-child behaviour only; a singleton's primary label is the change's own name, never its branch.

#### Scenario: Multi-instance child label uses branch when available

- **WHEN** a multi-instance logical change's child instance is on a named branch
- **THEN** the child row's primary label is the branch name
- **AND** the child row remains a single line

#### Scenario: Multi-instance child label falls back to path basename

- **WHEN** a multi-instance child instance's worktree is not on a named branch (detached HEAD or no git context)
- **THEN** the child row's primary label is the basename of the worktree path

#### Scenario: Progress and modification time are shown on the child row

- **WHEN** a multi-instance child row is rendered for a change with at least one incomplete task
- **THEN** the child row shows the instance's task progress as a fill meter (an outlined track with a fill whose width is `completedTasks / totalTasks`), with **no** inline digits
- **AND** the exact count is available via the meter's `title` tooltip ("N of M tasks") and its `role="progressbar"` aria attributes
- **AND** the child row shows a relative modification time (e.g. `12m ago`) in its trailing meta slot

### Requirement: Default Expansion of Tree Nodes

Every collapsible node in the workspace tree SHALL be rendered with a default expansion state derived from the node's current data at render time, with no prior interaction required.

For most node types — Repo groups, flat workspaces, multi-instance logical-change parents, instance rows, the Proposal/Design/Specs artifact nodes, capability rows under Specs, and individual task rows — the default SHALL be "expanded".

The **Tasks artifact node** SHALL default to "collapsed" whenever it is collapsible (its change has at least one section), regardless of task-completion state. This keeps a change's task rows out of the tree until the user opts into them by expanding the Tasks node; the node's task-progress meter or completion ✓ remains visible in its meta slot while collapsed (see *Tasks Artifact Node Progress*).

For one further node type the default depends on completion state:

- A **Section node** SHALL default to "collapsed" when its section has at least one task and every task in it is complete; otherwise it SHALL default to "expanded".

The user MAY override a node's default in either direction by clicking its disclosure caret. The application records overrides in two independent sets:

- A `collapsed` set of node IDs the user has closed against a default-open node.
- An `expanded` set of node IDs the user has opened against a default-closed node.

A node's rendered open/closed state SHALL be computed as follows:

- For a node whose current default is "open": open iff its ID is **not** in the `collapsed` set.
- For a node whose current default is "closed": open iff its ID **is** in the `expanded` set.

Because the Tasks artifact node's default is now "closed" unconditionally, a user who opens it has the node's ID recorded in the `expanded` set, and that preference persists across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: First-ever launch shows the tree expanded except for Tasks nodes and completed Sections

- **WHEN** the user launches the application for the first time after this change ships
- **AND** at least one workspace has been registered
- **THEN** every collapsible row whose computed default is "expanded" is rendered open
- **AND** every collapsible Tasks artifact node is rendered collapsed, regardless of task-completion state
- **AND** every Section node whose section has at least one task and all of them complete is rendered collapsed
- **AND** every section with at least one incomplete task carries the "expanded" default, so its task rows are visible once its Tasks node is expanded

#### Scenario: New change appears with its Tasks node collapsed

- **WHEN** the workspace tree is already rendered
- **AND** a new change directory is added to a registered workspace on disk, triggering a `change-added` event
- **THEN** the new change's row appears in the tree expanded
- **AND** the change's Proposal, Specs, and Design artifact rows are rendered expanded
- **AND** the change's Tasks artifact row is rendered collapsed
- **AND** each Section under Tasks carries the per-node-type Section default (collapsed iff all of its tasks are complete) for when the Tasks node is expanded

#### Scenario: Promoted multi-instance parent appears expanded

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the new disclosure parent row is rendered expanded by default
- **AND** the two instance rows beneath it are visible without any user click

#### Scenario: User expand overrides the collapsed Tasks node

- **WHEN** a Tasks artifact node is rendered collapsed by default
- **AND** the user clicks the Tasks row's disclosure caret
- **THEN** the Tasks node is re-rendered expanded
- **AND** its Section rows are visible, each per the Section default rule

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

The workspace tree SHALL auto-collapse **Section nodes** when their work is complete, so the user's attention is drawn to in-progress work. The Tasks artifact node is collapsed by default unconditionally (see *Default Expansion of Tree Nodes*) and therefore does not participate in this completion-based rule.

- A **Section node** is considered complete when its section has at least one task (`tasks.length > 0`) and every task in it is complete.

When a Section is complete, its default expansion state SHALL be "collapsed". When it is not complete (or has no tasks at all), its default expansion state SHALL be "expanded". The default is recomputed on every render from the node's current data, so transitions between in-progress and complete take effect within the watcher's debounce window with no extra user action.

The completion-based auto-collapse rule SHALL apply only to Section nodes. Change rows, Instance rows, Repo groups, flat workspaces, multi-instance logical-change parents, the Proposal/Specs/Design artifact rows, capability rows under Specs, and individual task rows SHALL continue to default to "expanded" regardless of completion state. The Tasks artifact node SHALL default to "collapsed" regardless of completion state.

A user override (a click on the disclosure caret) SHALL take precedence over the default and SHALL persist across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: Tasks artifact node defaults collapsed regardless of completion

- **WHEN** a Tasks artifact node is collapsible (its change has at least one section)
- **AND** the user has not explicitly expanded it
- **THEN** the Tasks artifact node is rendered collapsed whether the change's tasks are all complete, partially complete, or all incomplete
- **AND** its meta slot shows the task-progress meter (in progress) or the trailing `✓` (complete) per *Tasks Artifact Node Progress*

#### Scenario: Tasks node with sections but no parseable tasks still defaults collapsed

- **WHEN** a change's `tasks.md` has at least one section heading but no parseable task lines (`totalTasks === 0`)
- **THEN** the Tasks artifact node is collapsible and is rendered collapsed by default
- **AND** its meta slot shows neither a progress meter nor a `✓` glyph

#### Scenario: Section collapses when all its tasks complete

- **WHEN** a Section has at least one task and every task in it is complete
- **AND** the user has not explicitly expanded that Section since it became complete
- **THEN** the Section node is rendered collapsed

#### Scenario: Section stays expanded when partially complete

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section node is rendered expanded by default

#### Scenario: Section with no tasks is unaffected by the auto-collapse rule

- **WHEN** a Section has zero tasks
- **THEN** the Section row is rendered as a leaf (no chevron), as it is today
- **AND** the auto-collapse rule does not apply

#### Scenario: Completing the last task in a change does not change its Tasks node expansion

- **WHEN** a Tasks artifact node is rendered collapsed by default
- **AND** an external edit to `tasks.md` marks the change's last incomplete task complete
- **AND** the watcher emits the update
- **THEN** the Tasks artifact node remains collapsed (it was already collapsed by default)
- **AND** its meta slot swaps the progress meter for the trailing `✓` within the watcher debounce window

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

#### Scenario: User can expand a collapsed Tasks node or auto-collapsed Section

- **WHEN** a Tasks artifact node (collapsed by default) or a Section node (collapsed because its tasks are all complete) is rendered collapsed
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

### Requirement: Leaf-Task Completion Rendering

The tree pane SHALL render each leaf-task row using only its label text, with no leading completion glyph in either the completed or the pending state. A completed task (a `- [x]` line) SHALL render its label with a line-through text decoration AND the faint/dimmed task text colour. A pending task (a `- [ ]` line) SHALL render its label with no text decoration in the default task-label colour. The completion state of a leaf task SHALL be conveyed by this text treatment alone, and SHALL NOT be conveyed by a leading checkbox or checkmark glyph.

This requirement governs leaf-task rows only. It SHALL NOT alter the aggregate completion indicators defined elsewhere in this capability: the trailing `✓` completion glyph on a fully-complete Section, flat-Change, and per-Instance row, and the task-progress meter (see *Task Progress Meter* in `visual-identity`), all remain unchanged.

#### Scenario: Completed leaf task renders struck-through and dimmed

- **WHEN** a Section node is expanded and one of its task lines is `- [x]`
- **THEN** that task's row renders its label with a line-through text decoration and the dimmed task text colour
- **AND** no leading checkbox or checkmark glyph is rendered on the row

#### Scenario: Pending leaf task renders plain

- **WHEN** a Section node is expanded and one of its task lines is `- [ ]`
- **THEN** that task's row renders its label with no text decoration in the default task-label colour
- **AND** no leading checkbox or checkmark glyph is rendered on the row

#### Scenario: Aggregate completion indicators are retained

- **WHEN** every task in a Section is complete (and likewise for a fully-complete flat-Change row or per-Instance row)
- **THEN** the Section / flat-Change / Instance row continues to render its trailing `✓` completion glyph as before
- **AND** the task-progress meter continues to depict progress unchanged (per the *Task Progress Meter* requirement)

#### Scenario: Selection composes with the strikethrough treatment

- **WHEN** a completed leaf-task row is the currently selected node
- **THEN** the row shows the standard selection treatment
- **AND** the row's label remains struck-through and rendered in the dimmed task text colour

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

### Requirement: Settings Entrypoint in Sidebar Footer

The Settings entrypoint SHALL be rendered as a labeled row pinned to the bottom of the tree-navigation (left) pane. The row SHALL contain an icon and the visible text label "Settings". The row SHALL remain visible regardless of the scroll position of the workspace tree above it.

Clicking the row SHALL toggle the right pane between the workspace-tree's detail view and the Settings view, preserving the existing toggle semantics (a second click while Settings is open returns the right pane to its prior detail-view target).

The row SHALL convey its current state visually: when Settings is open in the right pane, the row SHALL render in an active treatment distinct from its idle state, mirroring the established active-affordance vocabulary already used elsewhere in the application chrome.

The Settings entrypoint SHALL NOT be rendered as a floating button overlaying the master-detail surface. No Settings affordance SHALL appear in the top-right corner of the application window.

#### Scenario: Footer row is visible at startup

- **WHEN** the user opens the main window
- **THEN** a row labeled "Settings" with an icon is rendered at the bottom of the left sidebar
- **AND** no floating Settings button is rendered in the top-right corner of the window

#### Scenario: Footer row stays pinned while the tree scrolls

- **WHEN** the workspace tree contains more rows than fit in the sidebar's height
- **AND** the user scrolls the tree to its midpoint or end
- **THEN** the Settings row remains visible at the bottom of the sidebar without scrolling out of view

#### Scenario: Clicking the row opens Settings

- **WHEN** the user clicks the Settings row while the right pane is showing a detail view
- **THEN** the right pane swaps to the Settings view
- **AND** the Settings row renders in its active state

#### Scenario: Clicking the row again closes Settings

- **WHEN** the user clicks the Settings row while Settings is already open in the right pane
- **THEN** the right pane returns to its prior detail-view target
- **AND** the Settings row returns to its idle state

#### Scenario: Selecting a tree node while Settings is open closes Settings

- **WHEN** Settings is open in the right pane
- **AND** the user clicks a renderable tree node (instance, artifact, spec, section, or task)
- **THEN** the right pane swaps to that node's detail view
- **AND** the Settings row returns to its idle state

### Requirement: Proposal Title Extraction

The title of a change SHALL be extracted from its `proposal.md` as follows. The parser SHALL skip ignorable preamble at the top of the document: blank lines, one leading YAML frontmatter block (when the first content line is exactly `---`, through its closing `---`), and HTML comment blocks (`<!--` through `-->`, single- or multi-line). The first content line after the preamble SHALL yield a title only when it is a level-1 Markdown heading — a single `#` followed by whitespace and non-empty text after trimming leading whitespace. An optional case-insensitive `Proposal:` prefix SHALL be stripped from the heading text, and the result trimmed. Any other first content line — a deeper heading such as `## Why`, body text, or an unterminated preamble block — SHALL yield no title, and the parser SHALL NOT examine any further line of the document. A change with no extractable title SHALL continue to be labelled by its change ID wherever titles are displayed (sidebar rows, archive browser, dashboard). A missing or unreadable `proposal.md` SHALL yield no title.

#### Scenario: Title on the first line parses as before

- **WHEN** a `proposal.md` begins with `# Add User Auth` on line 1
- **THEN** the extracted title is "Add User Auth"
- **AND** a legacy `# Proposal: Add User Auth` first line also yields "Add User Auth"

#### Scenario: Title found below ignorable preamble

- **WHEN** a `proposal.md` opens with blank lines, a YAML frontmatter block, or HTML comments (in any combination), followed by `# Add User Auth`
- **THEN** the extracted title is "Add User Auth"

#### Scenario: Template-faithful proposal yields no title

- **WHEN** a `proposal.md` follows the spec-driven template and its first content line is `## Why`
- **THEN** no title is extracted (never "Why")
- **AND** the change's rows display its change ID

#### Scenario: Non-heading first content line yields no title

- **WHEN** the first content line after the preamble is body text, a deeper heading, or `#` without a following space
- **THEN** no title is extracted and no later line of the document is considered
- **AND** an h1 appearing only later in the body (for example inside a fenced code block) is never mistaken for the title

### Requirement: Two-Line Sole-Change-Row Layout

A change row that is the **sole row for its change** SHALL render across two stacked lines within a single selectable row. Exactly two row types are sole change rows:

- a **flattened singleton instance row** — a git logical change with exactly one instance, rendered flat (no disclosure parent) per *Singleton Logical-Change Flattening and Promotion*; and
- a **flat-workspace change row** — a `ChangeData` row rendered directly under a non-git workspace node.

Multi-instance child rows (governed by *Instance Row Chrome*), multi-instance logical-change disclosure parents, Repo-group and workspace header rows, the Proposal/Specs/Design/Tasks artifact rows, capability rows, Section rows, and task rows are all excluded and SHALL remain single-line.

**Line 1 (primary).** Line 1 SHALL display the change's `proposal.md` title when one is extractable (see *Proposal Title Extraction*) — falling back, for a git singleton, to the logical change name, and for a flat-workspace change row, to its directory name. When a git singleton's line 1 shows the proposal title, the row SHALL expose the logical change name via its hover tooltip so the directory identity stays recoverable. The label SHALL render with slightly heavier weight than its artifact-row siblings so it reads as the row's heading, and SHALL own the full row width so it is no longer truncated by a trailing branch chip or status meta; it SHALL ellipsize against the row edge only when it alone exceeds the available width. Line 1 carries no worktree identity, swatch, or colour tint on its text.

**Line 2 (detail).** Line 2 SHALL render at the tree's dense meta type tier, visually subordinate, and SHALL be indented to begin at line 1's text origin (past the chevron) so it reads as belonging to the row above it. Line 2 SHALL place worktree identity on its leading edge and status on its trailing edge:

- **Leading edge.** For a git singleton row the leading edge SHALL show the instance's branch name as an outlined chip (per *visual-identity → Outlined Chip Badges*) tinted to the owning workspace's palette colour — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour. When the branch is not known (detached HEAD, bare worktree), the chip SHALL show the worktree folder basename instead. A flat-workspace change row has no git worktree identity; in its place the leading edge SHALL show the change's identifier (`changeId`), the same identifier the row shows today.
- **Status (trailing).** Line 2 SHALL carry the row's existing status elements, with their existing presence rules, on its trailing edge. For a **git singleton row** these are the task-progress meter while work is in progress or the completion ✓ when every task is complete (per *Change-Row Completion Glyph* and *Tasks Artifact Node Progress*), the relative modification time, and the divergence label when present (per *Per-Instance Divergence Label*). For a **flat-workspace change row** the only status element is the completion ✓ when every task is complete; a flat-workspace row carries no progress meter, modification time, or divergence label. The active-instance indicator is a multi-instance-child element and SHALL NOT appear on a sole change row.

**Workspace-colour rail.** A sole change row SHALL tint its inline-start border — the 2px slot the selection bar occupies — with the owning workspace's palette colour, so each change reads as belonging to its workspace and the colour ties the row to its branch chip top-to-bottom. While the row is selected the selection bar (the 2px `--accent` border, per *visual-identity → Tree Row Selection Model*) SHALL take precedence and replace the rail; the rail SHALL reappear when the row is deselected. A workspace with no configured palette colour renders no rail. Header rows and the other excluded row types do not render the rail.

**One interaction unit.** The two lines SHALL form a single interaction unit: one click target that selects the change and one selection unit. The selection treatment (the 2px `--accent` inline-start bar plus its tint wash) and the hover wash SHALL span both lines. The disclosure chevron SHALL toggle the row's artifact subtree exactly as it does today and SHALL remain associated with the row as a whole.

#### Scenario: Git singleton renders its proposal title on the first line

- **WHEN** a git logical change has exactly one instance and its `proposal.md` yields a title
- **THEN** line 1 shows that title across the full row width, in a slightly heavier weight than the artifact rows below it
- **AND** the label is not truncated by any branch or status element on the same line
- **AND** the row's hover tooltip carries the logical change name

#### Scenario: Git singleton without an extractable title falls back to the change name

- **WHEN** a git logical change has exactly one instance and its `proposal.md` is missing or yields no title
- **THEN** line 1 shows the logical change name, exactly as before

#### Scenario: Branch appears on the detail line as a workspace-tinted chip

- **WHEN** a git singleton instance's worktree is on a named branch
- **THEN** line 2 shows the branch name as an outlined chip on its leading edge, with chip text and border tinted to the owning workspace's palette colour (a contrast-safe shade)
- **AND** line 2 shows the task-progress meter (or completion ✓) and relative modification time on its trailing edge

#### Scenario: Detached-HEAD singleton shows the folder basename only

- **WHEN** a git singleton instance's worktree is not on a named branch
- **THEN** line 2's worktree-identity segment shows the worktree folder basename alone, with no branch name

#### Scenario: Flat-workspace change row uses two lines with a meta-only detail line

- **WHEN** a change is rendered as a flat-workspace change row under a non-git workspace node
- **THEN** line 1 shows the change's title (or its change-id when no title is present)
- **AND** line 2 shows the change's `changeId` on its leading edge and the completion ✓ (when complete) on its trailing edge, with no branch, worktree folder, progress meter, modification time, or divergence label

#### Scenario: Multi-instance child row is excluded and stays single-line

- **WHEN** a logical change has two or more instances and is rendered as a disclosure parent with child rows
- **THEN** each child row remains a single line per *Instance Row Chrome*
- **AND** no child row adopts the two-line layout

#### Scenario: Completed sole change row shows its completion glyph on the detail line

- **WHEN** a sole change row's change has at least one task and every task is complete
- **THEN** line 2's trailing edge shows the completion ✓ in place of the progress meter

#### Scenario: Selection and hover span both lines of a sole change row

- **WHEN** a sole change row is selected, or the pointer hovers over either of its two lines
- **THEN** the selection bar and tint (or the hover wash) cover both lines as one contiguous row
- **AND** a click anywhere on either line selects the change and updates the detail pane

#### Scenario: Workspace-colour rail marks each change row

- **WHEN** a sole change row is rendered for a workspace that has a configured palette colour
- **AND** the row is not selected
- **THEN** the row's inline-start border (the selection-bar slot) is tinted to that workspace's palette colour
- **AND** every change row under the same workspace shares that colour, matching the workspace's top-level swatch

#### Scenario: Selection bar overrides the rail

- **WHEN** a sole change row that is showing its workspace-colour rail becomes selected
- **THEN** the inline-start border renders the 2px `--accent` selection bar instead of the workspace colour
- **AND** the workspace-colour rail reappears once the row is deselected

### Requirement: Archive Entrypoint in Sidebar Footer

The Archive entrypoint SHALL be rendered as a labeled row in the bottom region of the tree-navigation (left) pane, directly above the Settings entrypoint (see *Settings Entrypoint in Sidebar Footer*). The row SHALL contain an icon and the visible text label "Archive". The row SHALL remain visible regardless of the scroll position of the workspace tree above it.

Clicking the row SHALL toggle the right pane between its prior detail target and the **Archive view** (see the *Archive View* requirement in the `archive-browser` capability), preserving the same toggle semantics the Settings entrypoint uses: a second click while the Archive view is open returns the right pane to its prior detail target, and selecting a renderable tree node, opening Settings, or opening the Dashboard closes the Archive view.

The row SHALL convey its current state visually: when the Archive view is open in the right pane, the row SHALL render in an active treatment distinct from its idle state, mirroring the active-affordance vocabulary used by the Settings and Dashboard entrypoints.

The Archive entrypoint SHALL NOT be rendered as a floating button overlaying the master-detail surface, and SHALL NOT display a count badge — no archive content is computed until the view is opened.

#### Scenario: Archive row is visible in the sidebar footer

- **WHEN** the user opens the main window
- **THEN** a row labeled "Archive" with an icon is rendered in the sidebar footer, above the "Settings" row
- **AND** no floating Archive button is rendered over the master-detail surface
- **AND** the Archive row displays no count badge

#### Scenario: Clicking the row opens the Archive view

- **WHEN** the user clicks the Archive row while the right pane is showing a detail target
- **THEN** the right pane swaps to the Archive view
- **AND** the Archive row renders in its active state

#### Scenario: Clicking the row again closes the Archive view

- **WHEN** the user clicks the Archive row while the Archive view is already open
- **THEN** the right pane returns to its prior detail target
- **AND** the Archive row returns to its idle state

#### Scenario: Selecting a tree node closes the Archive view

- **WHEN** the Archive view is open in the right pane
- **AND** the user selects a renderable tree node (instance, artifact, spec, section, or task)
- **THEN** the right pane swaps to that node's detail view
- **AND** the Archive row returns to its idle state

### Requirement: Workspace Tree Keyboard Navigation

The workspace tree SHALL be fully operable from the keyboard as a WAI-ARIA tree with a roving tabindex: the tree occupies exactly one position in the window's Tab order, and within it a single current row carries focus, movable with the keyboard. Keyboard activation SHALL reuse the same selection contract as pointer clicks — a row whose click renders content in the detail pane renders the same content when activated by keyboard, and rows whose clicks are disclosure-only remain disclosure-only.

#### Scenario: Tree is a single Tab stop with a roving current row

- **WHEN** the user presses Tab from the control preceding the tree (or Shift+Tab from the control following it)
- **THEN** focus lands on the tree's current row — the last row focused in this session, or the first visible row if none — rather than entering every row in sequence
- **AND** pressing Tab again moves focus out of the tree to the next control in the window's Tab order

#### Scenario: Arrow keys traverse visible rows

- **WHEN** the tree has focus and the user presses ArrowDown or ArrowUp
- **THEN** focus moves to the next or previous visible row in rendered order, crossing workspace boundaries, without wrapping at either end
- **AND** the newly focused row scrolls into view if it is outside the sidebar's viewport

#### Scenario: Home and End jump to the extremes

- **WHEN** the tree has focus and the user presses Home or End
- **THEN** focus moves to the first or last visible row of the tree

#### Scenario: ArrowRight and ArrowLeft drive disclosure and parent jumps

- **WHEN** the user presses ArrowRight on a collapsed expandable row
- **THEN** the row expands, honouring the same expansion-persistence behavior as a chevron click
- **WHEN** the user presses ArrowRight on an already-expanded row
- **THEN** focus moves to the row's first child
- **WHEN** the user presses ArrowLeft on an expanded row
- **THEN** the row collapses
- **WHEN** the user presses ArrowLeft on a collapsed or leaf row that has a parent row
- **THEN** focus moves to the parent row

#### Scenario: Enter and Space activate the current row

- **WHEN** the user presses Enter or Space on a row whose pointer click renders content in the detail pane (instance, proposal/design/tasks artifact, capability-spec, section, and task rows)
- **THEN** the row is selected and the detail pane renders exactly what a pointer click on that row would render
- **WHEN** the user presses Enter or Space on a disclosure-only grouping row (workspace, repo, logical change, and change rows, plus the Specs artifact row — whose pointer click also renders no content)
- **THEN** the row's expansion toggles, identically to a chevron click

#### Scenario: Debounced follow-focus opens content without per-keystroke reads

- **WHEN** keyboard focus comes to rest on a row whose pointer click renders content in the detail pane, and remains there for a short settle delay (approximately 150 ms)
- **THEN** the detail pane renders that row's content as if the row had been activated
- **WHEN** focus passes over such rows more quickly than the settle delay (for example while an arrow key is held down)
- **THEN** no intermediate row's content is loaded or rendered
- **WHEN** keyboard focus rests on a disclosure-only grouping row
- **THEN** the detail pane does not change

#### Scenario: First-letter typeahead

- **WHEN** the tree has focus and the user types a printable character
- **THEN** focus moves to the next visible row after the current one whose label starts with that character, comparing case-insensitively and wrapping past the end of the tree
- **AND** if no visible row label starts with that character, focus does not move

#### Scenario: Tree rows expose ARIA tree semantics

- **WHEN** the tree is rendered
- **THEN** the container exposes `role="tree"`, every row exposes `role="treeitem"` with an accurate `aria-level`, expandable rows expose `aria-expanded` reflecting their disclosure state, the selected row exposes `aria-selected="true"`, and nested child groups are wrapped in `role="group"` containers
- **AND** dim missing-artifact rows remain keyboard-focusable but expose `aria-disabled="true"` and do not respond to activation

#### Scenario: Focus survives the focused row disappearing

- **WHEN** a tree refresh (for example a filesystem cache event) removes the row that currently holds keyboard focus
- **THEN** focus falls back to the nearest surviving ancestor row derived from the removed row's hierarchical node ID, rather than being lost to the document body

#### Scenario: Keyboard focus movement does not re-render the whole tree

- **WHEN** the user moves keyboard focus between rows
- **THEN** only the rows whose visual state changed re-render; unaffected subtrees are not re-rendered

### Requirement: Shell Keyboard Operability

The browsing shell around the tree SHALL be keyboard-operable: split-pane dividers MUST be focusable and resizable from the keyboard, the Settings and Archive panes MUST be dismissible with Escape, and every keyboard-focusable control in the shell MUST show a visible focus indicator when focused via keyboard, using the visual-identity spec's keyboard-focus recipe.

#### Scenario: Dividers resize from the keyboard

- **WHEN** a split-pane divider receives keyboard focus and the user presses ArrowLeft or ArrowRight
- **THEN** the adjacent pane resizes by a fixed step per keypress, respecting the same minimum-width limits as a pointer drag, and the divider exposes `role="separator"` with `aria-valuenow`, `aria-valuemin`, and `aria-valuemax` reflecting the current and permitted sizes

#### Scenario: Escape dismisses Settings and Archive

- **WHEN** the Settings pane or the Archive pane is open and the user presses Escape (with no text input focused that consumes it)
- **THEN** the open pane closes and the detail pane returns to what it previously displayed

#### Scenario: Focusable controls show visible keyboard focus

- **WHEN** any focusable control in the sidebar, archive view, graph rail, or settings view receives focus via keyboard
- **THEN** it renders a visible focus indicator per the visual-identity keyboard-focus recipe
- **AND** focus styles use `:focus-visible` so pointer clicks do not paint focus rings

### Requirement: Working-Tree Status Indicators

The tree pane SHALL surface git working-tree status for git-backed repositories
through two indicator families, leaving non-git (flat) workspaces unchanged.

On each repository node, when the repository's dirty rollup is set, the tree
SHALL render a whole-repo **dirty** indicator; when the repository additionally
has uncommitted specs, the tree SHALL render a **distinct** specs-uncommitted
indicator alongside it, so that an uncommitted source file is visually
distinguishable from an uncommitted spec. Both indicators SHALL be absent when
the repository is clean.

On each change-instance row, the tree SHALL render a commit-state chip when the
instance's spec commit state is `Modified` or `Untracked`, positioned alongside
the existing divergence chip. A `Committed` instance SHALL render no such chip.

#### Scenario: Repo with an uncommitted spec shows both rollup indicators

- **WHEN** a repository node renders and the repository has a worktree with an
  untracked or modified change directory
- **THEN** the node shows the whole-repo dirty indicator
- **AND** the node shows the distinct specs-uncommitted indicator

#### Scenario: Repo dirty only from non-spec files shows one indicator

- **WHEN** a repository is dirty solely from files outside `openspec/`
- **THEN** the node shows the whole-repo dirty indicator
- **AND** the node does not show the specs-uncommitted indicator

#### Scenario: Clean repo shows no indicators

- **WHEN** a repository and all its worktrees are clean
- **THEN** the repository node shows neither indicator

#### Scenario: Untracked instance shows a commit-state chip

- **WHEN** a change-instance row renders for a worktree whose copy of the change
  is untracked
- **THEN** the row shows an "untracked" commit-state chip beside the divergence
  chip

#### Scenario: Committed instance shows no commit-state chip

- **WHEN** a change-instance row renders for a worktree whose copy of the change
  is fully committed
- **THEN** the row shows no commit-state chip

#### Scenario: Flat workspace is unaffected

- **WHEN** a non-git (flat) workspace renders in the tree
- **THEN** no working-tree status indicators are shown for it

### Requirement: Artifact Reads Are Confined to Registered Workspaces

Reading an OpenSpec artifact's markdown SHALL be authorized only when the workspace it is read from is a registered (or registry-discovered) workspace, and a caller-supplied workspace that is not in the registry SHALL be refused rather than read, even when the requested path resolves to a real `openspec/changes/…` file on disk. This authorization SHALL be applied in addition to the existing path-traversal guard (which keeps the resolved file within the workspace's `openspec/changes/` subtree): the traversal guard bounds *where within a workspace* a read may reach, and this requirement bounds *which workspaces* may be read at all. The workspace SHALL be matched by its canonical path against the registry's known workspace folders using the same canonicalization the registry keys on, and the check SHALL be enforced at the shared application boundary so it holds for every frontend and transport that can read artifacts.

#### Scenario: An artifact read against an unregistered workspace is refused

- **WHEN** an artifact-read is requested for a workspace path that is not a registered or registry-discovered workspace
- **THEN** the read is refused with an error
- **AND** no file under that path is read, even if an `openspec/changes/.../<artifact>.md` file exists there

#### Scenario: An artifact read against a registered workspace succeeds

- **WHEN** an artifact-read is requested for a change in a registered workspace
- **THEN** the artifact's markdown is returned as before, subject to the existing path-traversal guard

#### Scenario: The confinement holds across transports

- **WHEN** an artifact-read is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same registered-workspace requirement applies, because it is enforced at the shared application boundary

### Requirement: Mermaid Diagram Rendering

The detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every other fenced code block SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

A rendered diagram SHALL derive its colours and fonts from the application's design tokens (see the *Design Token Layer* and *Typography System* requirements in the `visual-identity` capability) so that it reads as part of the same surface as the surrounding prose in both the light and dark schemes. When the operating system colour scheme changes while a diagram is visible, the diagram SHALL re-render so its colours follow the active scheme.

A `mermaid` fence whose content is not valid diagram source SHALL degrade gracefully: the detail pane SHALL present the fence's raw source together with a quiet indication that the diagram could not be rendered, SHALL NOT blank or crash the pane, and SHALL NOT surface the diagram engine's own error graphic. The rest of the artifact SHALL render normally. Diagram rendering SHALL run under a strict security posture so that diagram source cannot inject active content (scripts or click-through handlers) into the application.

#### Scenario: A valid mermaid fence renders as a diagram

- **WHEN** an artifact contains a fenced code block with the `mermaid` info string and valid diagram source
- **THEN** the detail pane renders it as a graphical diagram
- **AND** the raw mermaid source text is not shown

#### Scenario: Other fenced code blocks are unaffected

- **WHEN** an artifact contains a fenced code block in another language (for example `rust` or `ts`)
- **THEN** it renders as syntax-highlighted source as before
- **AND** it is not treated as a diagram

#### Scenario: An invalid mermaid fence degrades to source

- **WHEN** an artifact contains a `mermaid` fence whose content is not valid diagram source
- **THEN** the detail pane shows the fence's raw source
- **AND** shows a quiet indication that the diagram could not be rendered
- **AND** the rest of the artifact still renders
- **AND** the diagram engine's default error graphic is not shown

#### Scenario: Diagrams follow the design tokens and colour scheme

- **WHEN** a diagram is rendered
- **THEN** its colours and font derive from the application's design tokens rather than the diagram engine's stock palette
- **AND** when the operating system switches between light and dark while the diagram is visible, the diagram re-renders with the active scheme's tokens

#### Scenario: Diagram source cannot inject active content

- **WHEN** a `mermaid` fence contains content that attempts to embed a script or a click-through handler
- **THEN** the rendered diagram contains no active content
- **AND** no script from the diagram source executes
