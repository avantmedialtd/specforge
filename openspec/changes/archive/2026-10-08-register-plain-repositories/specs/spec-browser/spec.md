## MODIFIED Requirements

### Requirement: Workspace Tree Hierarchy

The tree pane SHALL display tracked workspaces grouped by repository where applicable, as a tree of exactly **two levels**: top-level rows and change rows. For each git repository with at least one tracked workspace, the tree SHALL render a top-level Repo group node containing one row per active logical change. For each non-git workspace, the tree SHALL render a top-level workspace node containing that workspace's changes directly.

A logical change groups every `ChangeInstance` in one repository that shares the same **logical change identifier**: the change's directory name for an active instance, and that directory name with at most one leading `YYYY-MM-DD-` prefix removed for an archived one. The two forms are grouped together deliberately, so that an active instance and an archived instance of one change are comparable — which is what the *Per-Instance Divergence Label* requirement needs in order to report `[stale]` at all.

Grouping them, however, SHALL NOT put an archived instance in front of a consumer that renders active changes. Within a logical change, instances SHALL be partitioned by whether they are archived in their worktree, and only the non-archived partition counts as rendered. An archived instance is summarised from a directory listing rather than parsed — it carries no title, no task counts and no artifacts, because the archive is deliberately never read on the aggregation path (see *On-Demand, Off-Hot-Path Loading* in the `archive-browser` capability).

Inside a Repo group, each logical change SHALL be rendered as exactly **one change row**, whatever its rendered instance count. A change with one rendered instance renders as a two-line row naming its branch; a change with two or more renders the same single row carrying an instance-count chip in the branch chip's place (see *Two-Line Sole-Change-Row Layout*), and the instance to read is chosen in the change header (see *Instance Switcher in the Change Header*). An archived instance SHALL NOT contribute to that count.

A change row is a **leaf**. It SHALL expose no artifact, capability, section or task rows beneath it and no disclosure affordance. Activating a change row SHALL navigate to the change's **default artifact** of its **default instance**: the first artifact present on disk in the fixed order Proposal, Design, Tasks, then the first capability spec in listing order; and the main worktree's instance when it hosts the change, otherwise the first rendered instance in aggregation order. The change's other artifacts are reached from the change header (see *Artifact Tab Strip in the Change Header*). A change with no artifact present SHALL still be selectable, and the detail pane SHALL show its empty state.

The tree SHALL render **active changes only**. Archived logical changes SHALL NOT appear anywhere in the tree — neither in an Active section nor in a separate Archive section. Archived changes are browsed exclusively in the dedicated Archive view (see the *Archive View* requirement in the `archive-browser` capability).

A top-level row is a **disclosure row**, open by default. A top-level row the user closes SHALL stay closed for the rest of the session and SHALL NOT be persisted: no disclosure state of any kind survives a restart, and the tree performs no settings write on a toggle. A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — and opens the workspace file browser for the row's workspace in the detail pane (see the `workspace-file-browser` capability). No placeholder child row SHALL be rendered beneath an empty top-level row. A top-level row whose only changes are archived SHALL therefore render as a leaf with a `0` active count, the same as a row with no changes at all.

A top-level row that reports no OpenSpec (see the *OpenSpec Presence Is Derived on Every Aggregation* requirement in the `workspace-registry` capability) SHALL render a muted "no OpenSpec" marker **in place of** its count badge, so it is not mistaken for an OpenSpec repository with nothing active. The marker SHALL carry the accessible name "No OpenSpec folder". In every other respect the row is an empty top-level row:
- it is a leaf with no disclosure chevron;
- it is selectable and opens the workspace file browser;
- it keeps its display name, swatch, dirty indicator and position.

When the row later reports OpenSpec present, the marker SHALL give way to the count badge without a restart.

#### Scenario: Git repo with multiple worktrees shown as one Repo group

- **WHEN** a repository has three tracked worktrees, two of which contain a change with the same directory name
- **THEN** the tree shows one top-level Repo group for that repository
- **AND** the two-instance change appears as exactly one change row carrying an instance-count chip
- **AND** any single-instance change appears as one change row naming its branch

#### Scenario: An archived instance is not rendered beside its active twin

- **WHEN** a change is archived in one worktree of a repository and still active in another
- **THEN** the tree renders exactly one row for it, counting the active instance only
- **AND** that row carries no instance-count chip
- **AND** no unlabelled or artifact-less row is rendered for the archived copy

#### Scenario: Non-git workspace shown as a standalone top-level node

- **WHEN** a tracked workspace is not inside a git repository
- **THEN** the workspace is rendered as a top-level node (not a Repo group)
- **AND** the workspace's changes are rendered directly underneath without instance aggregation

#### Scenario: Activating a change row opens its default artifact

- **WHEN** the user activates the row of a change whose `proposal.md` is absent and whose `design.md` is present
- **THEN** the detail pane renders that change's design
- **AND** the change header's tab strip shows Design as the active tab

#### Scenario: Activating a multi-instance change row opens the default instance

- **WHEN** the user activates the row of a change hosted in the repository's main worktree and in one feature worktree
- **THEN** the detail pane renders the main worktree's copy of the default artifact
- **AND** the change header's instance switcher marks the main worktree's instance as selected

#### Scenario: A change row exposes nothing beneath it

- **WHEN** a change row is rendered
- **THEN** it has no disclosure chevron
- **AND** no artifact, capability, section or task row is rendered beneath it

#### Scenario: Archived logical changes are not shown in the tree

- **WHEN** every instance of a logical change is under `openspec/changes/archive/` in its worktree
- **THEN** the logical change is not shown anywhere in the tree
- **AND** it is browsable in the Archive view instead

#### Scenario: Empty top-level row renders as a leaf

- **WHEN** a top-level Repo group node or non-git workspace node that reports OpenSpec present has zero active changes
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
- **THEN** the row re-renders as an open disclosure parent
- **AND** the count badge advances from `0` to the new count

#### Scenario: Top-level disclosure does not survive a restart

- **WHEN** the user closes a top-level row and restarts the application
- **THEN** the row renders open
- **AND** no settings write was performed when it was closed

#### Scenario: A repository without OpenSpec shows the marker instead of a count

- **WHEN** a top-level Repo group node reports no OpenSpec
- **THEN** the row renders as a leaf with the "no OpenSpec" marker where the count badge would be
- **AND** no `0` count badge is rendered
- **AND** clicking the row opens the workspace file browser for the repository

#### Scenario: The marker gives way to the count when OpenSpec appears

- **WHEN** a row showing the "no OpenSpec" marker begins to report OpenSpec present
- **THEN** the marker is replaced by the count badge without a restart

