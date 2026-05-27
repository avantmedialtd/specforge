## MODIFIED Requirements

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
