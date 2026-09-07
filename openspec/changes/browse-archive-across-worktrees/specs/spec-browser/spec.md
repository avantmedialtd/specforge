## MODIFIED Requirements

### Requirement: Per-Instance Divergence Label

For every `ChangeInstance` that is not on the repository's default branch, the application SHALL compute and display at most one divergence label by comparing the instance's change directory contents against the default-branch instance of the same logical change. The labels are:

- `[diverged]` — the change exists in both the default-branch instance and the non-default instance, but the file contents differ at the byte level.
- `[stale]` — the change is archived on the default branch (under `openspec/changes/archive/`) but is still active in the non-default instance.

If the change does not exist on the default branch at all, or if no default branch is known, or if the contents are identical, the instance SHALL display no divergence label.

Two instances SHALL be recognised as instances of the same logical change whether or not they are archived, and whether or not their archive directories carry a date prefix. An archived instance's directory is named `<YYYY-MM-DD>-<id>` in ordinary use and `<id>` in the legacy un-dated form; the logical change it belongs to is identified by `<id>` in both cases. Comparing an active instance against an archived one SHALL therefore key on the change's bare identifier, never on the archive directory's raw name — keying the two forms differently makes the `[stale]` label unreachable for every dated archive directory, which is the form that occurs in practice.

#### Scenario: Diverged content gets the diverged label

- **WHEN** an instance on a non-default branch has different content under `openspec/changes/<name>/` than the default-branch instance of the same logical change
- **THEN** the instance row displays the `[diverged]` label

#### Scenario: Stale-vs-archive gets the stale label

- **WHEN** the default-branch instance of a logical change is in `openspec/changes/archive/<name>/`
- **AND** a non-default instance of the same logical change is in `openspec/changes/<name>/` (still active)
- **THEN** the non-default instance row displays the `[stale]` label

#### Scenario: Stale label fires against a dated archive directory

- **WHEN** the default-branch instance of a logical change `add-thing` is archived at `openspec/changes/archive/2026-09-05-add-thing/`
- **AND** a non-default instance is still active at `openspec/changes/add-thing/`
- **THEN** the non-default instance row displays the `[stale]` label
- **AND** the date prefix on the archive directory does not prevent the two instances from being recognised as the same logical change

#### Scenario: Branch-only change gets no label

- **WHEN** a logical change has no instance on the default branch (it was created only on a feature branch)
- **THEN** every non-default instance displays no divergence label

#### Scenario: Identical content gets no label

- **WHEN** a non-default instance has byte-identical content to the default-branch instance of the same logical change
- **THEN** the non-default instance row displays no divergence label

#### Scenario: No default branch produces no labels

- **WHEN** the repository has no detected default branch
- **THEN** no instance of any logical change in that repository displays a divergence label

### Requirement: Workspace Tree Hierarchy

The tree pane SHALL display tracked workspaces grouped by repository where applicable. For each git repository with at least one tracked workspace, the tree SHALL render a top-level Repo group node containing the repository's logical changes. For each non-git workspace, the tree SHALL render a top-level workspace node containing that workspace's changes directly, as before.

A logical change groups every `ChangeInstance` in one repository that shares the same **logical change identifier**: the change's directory name for an active instance, and that directory name with at most one leading `YYYY-MM-DD-` prefix removed for an archived one. The two forms are grouped together deliberately, so that an active instance and an archived instance of one change are comparable — which is what the *Per-Instance Divergence Label* requirement needs in order to report `[stale]` at all.

Grouping them, however, SHALL NOT put an archived instance in front of a consumer that renders active changes. Within a logical change, instances SHALL be partitioned by whether they are archived in their worktree, and only the non-archived partition is rendered as tree rows. An archived instance is summarised from a directory listing rather than parsed — it carries no title, no task counts and no artifacts, because the archive is deliberately never read on the aggregation path (see *On-Demand, Off-Hot-Path Loading* in the `archive-browser` capability) — so rendering one would produce an unlabelled row with an empty artifact subtree and nothing to open.

Inside a Repo group, each logical change is rendered according to its **rendered instance count**: a logical change with exactly one rendered instance SHALL be rendered as a flat instance row with no parent disclosure; a logical change with two or more SHALL be rendered as a disclosure parent row with one child row per rendered instance. An archived instance SHALL NOT contribute to that count, so a change that is active in exactly one worktree renders as a flat row whether or not another worktree holds an archived copy of it.

Each `ChangeInstance` row, when rendered, SHALL expose the same four artifact nodes — Proposal, Specs, Design, Tasks — in fixed order, mirroring the existing artifact subtree. The Specs node, when present, contains one child per capability spec file. The Tasks node, when present, contains one child per section in that instance's `tasks.md`, and each section contains one child per task line.

The tree SHALL render **active changes only**. Archived logical changes SHALL NOT appear anywhere in the tree — neither in an Active section nor in a separate Archive section. Archived changes are browsed exclusively in the dedicated Archive view (see the *Archive View* requirement in the `archive-browser` capability).

A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — and opens the workspace file browser for the row's workspace in the detail pane (see the `workspace-file-browser` capability). No placeholder child row SHALL be rendered beneath an empty top-level row. A top-level row whose only changes are archived SHALL therefore render as a leaf with a `0` active count, the same as a row with no changes at all.

#### Scenario: Git repo with multiple worktrees shown as one Repo group

- **WHEN** a repository has three tracked worktrees, two of which contain a change with the same directory name
- **THEN** the tree shows one top-level Repo group for that repository
- **AND** the two-instance change appears under a disclosure parent row with both instances as children
- **AND** any single-instance change appears as a flat row directly under the Repo group

#### Scenario: An archived instance is not rendered beside its active twin

- **WHEN** a change is archived in one worktree of a repository and still active in another
- **THEN** the tree renders exactly one row for it — the active instance
- **AND** that row is a flat row, not a two-instance disclosure parent
- **AND** no unlabelled or artifact-less row is rendered for the archived copy

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
