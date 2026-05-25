# spec-browser

## MODIFIED Requirements

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

## ADDED Requirements

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
