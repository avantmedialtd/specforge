## ADDED Requirements

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

## MODIFIED Requirements

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

A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — without changing the detail pane (consistent with the existing `repo` / `workspace` selection contract for grouping nodes). No placeholder child row SHALL be rendered beneath an empty top-level row. A top-level row whose only changes are archived SHALL therefore render as a leaf with a `0` active count, the same as a row with no changes at all.

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
- **AND** the detail pane is not changed by the click (consistent with the existing selection contract for `repo` and `workspace` grouping nodes)
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
