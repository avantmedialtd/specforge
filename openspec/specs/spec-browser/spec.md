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

The tree pane and the detail pane SHALL reflect on-disk changes within the watcher's debounce window without requiring user action.

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

Every collapsible node in the workspace tree SHALL be rendered expanded by default the first time the user encounters it, with no prior interaction required. This applies at every depth — Repo groups, flat workspaces, multi-instance logical-change parents, instance rows, artifact nodes (Specs, Tasks), capability rows under Specs, sections under Tasks — down to and including individual task rows.

The expansion state SHALL be modelled as the set of node IDs the user has explicitly collapsed. A node SHALL be rendered as open if and only if its ID is **not** in that set. Toggling a node adds its ID to the set if absent or removes it if present.

#### Scenario: First-ever launch shows the tree fully expanded

- **WHEN** the user launches the application for the first time after this change ships
- **AND** at least one workspace has been registered
- **THEN** every collapsible row in the workspace tree is rendered in its expanded state
- **AND** the artifact rows (Proposal, Specs, Design, Tasks) of every change are visible without further interaction
- **AND** the Tasks artifact's section rows and each section's task rows are visible without further interaction

#### Scenario: New change appears expanded after watcher event

- **WHEN** the workspace tree is already rendered
- **AND** a new change directory is added to a registered workspace on disk, triggering a `change-added` event
- **THEN** the new change's row appears in the tree
- **AND** the change's artifact rows, section rows, and task rows are all rendered expanded without any user click

#### Scenario: Promoted multi-instance parent appears expanded

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the new disclosure parent row is rendered expanded by default
- **AND** the two instance rows beneath it are visible without any user click

### Requirement: User Collapse State Persists Across Sessions

When the user collapses a tree node, the application SHALL persist that collapse so that, after quitting and relaunching, the same node is rendered collapsed without further user action. When the user re-expands a previously-collapsed node, the application SHALL persist that re-expansion so that, after relaunching, the node is again rendered expanded.

The persisted state SHALL be the set of node IDs that the user has collapsed. Nodes whose IDs are not in the set SHALL be rendered expanded, including nodes whose IDs were never observed in any previous session (e.g., changes or sections created after the persisted set was last written).

#### Scenario: Collapsed node stays collapsed after restart

- **WHEN** the user collapses an artifact row (e.g., the Tasks node of some change)
- **AND** the user quits and relaunches the application
- **THEN** the same artifact row is rendered in its collapsed state in the restored tree
- **AND** sibling nodes the user did not collapse remain expanded

#### Scenario: Re-expanded node stays expanded after restart

- **WHEN** the user has previously collapsed and persisted a node
- **AND** the user re-expands that node in the current session
- **AND** the user quits and relaunches the application
- **THEN** the node is rendered in its expanded state in the restored tree

#### Scenario: Settings file with no persisted collapses loads cleanly

- **WHEN** the user launches a version of the application that supports persisted collapses for the first time
- **AND** the existing settings file on disk was written by a previous version with no field for collapsed node IDs
- **THEN** the application loads the settings file successfully
- **AND** the tree is rendered with every node expanded (empty collapsed set)

#### Scenario: Persistence write is bounded by user toggles

- **WHEN** the user toggles the same node open and closed several times in rapid succession
- **THEN** the persisted state eventually reflects the final toggled position
- **AND** the application does not write a settings file for every intermediate state (writes are coalesced)

### Requirement: Tree Expansion Has No First-Sight Auto-Expansion Effect

The application SHALL NOT maintain a separate "first time we see this node, mark it expanded" code path. The default-expanded behaviour is a consequence of the inverted state model (collapses tracked, opens implicit), not of any seeding logic that runs on view changes.

#### Scenario: No second-mount re-seeding

- **WHEN** the watcher emits a `cache-updated` event that causes the tree's `views` prop to re-render
- **THEN** the application does not run any effect that adds node IDs to an "expanded" set in response

#### Scenario: User-collapse survives a tree re-render

- **WHEN** the user has collapsed a node
- **AND** the watcher subsequently emits a `cache-updated` event for that workspace
- **THEN** the node remains collapsed after the re-render
- **AND** the application does not re-open the node as a side effect of the view change
