# working-tree-status

## ADDED Requirements

### Requirement: Per-Worktree Working-Tree Status

For every tracked git worktree, the application SHALL compute the worktree's
git working-tree status with a single porcelain invocation
(`git status --porcelain --untracked-files=all`) issued through the application's
git command chokepoint, so that the WSL backend is used transparently where it
applies. The computation SHALL yield, from that one invocation, both a
whole-worktree dirty bit and a per-change classification, and SHALL degrade to a
clean/unknown result (no dirty bit, no per-change state) when git is unavailable
or the command fails.

#### Scenario: Clean worktree

- **WHEN** a worktree has no staged, unstaged, or untracked changes
- **THEN** its computed status is not dirty
- **AND** every change in that worktree classifies as committed

#### Scenario: Status survives a missing git binary

- **WHEN** the status computation cannot run git for a worktree
- **THEN** the worktree is reported as not dirty with no per-change state
- **AND** no error is surfaced to the user for that worktree

### Requirement: Per-Change Spec Commit State

Each change instance (one worktree's copy of a logical change) SHALL carry a spec
commit state of `Committed`, `Modified`, or `Untracked`, derived from the lines of
the worktree's porcelain status whose path lies under that change's
`openspec/changes/<id>/` directory. Classification SHALL follow the precedence:
any tracked modification yields `Modified`; otherwise any untracked entry yields
`Untracked`; otherwise `Committed`. Rename destinations and quoted paths SHALL be
resolved before the prefix test.

#### Scenario: Brand-new spec that only exists in a worktree

- **WHEN** a change directory exists in a worktree but is entirely untracked by
  git
- **THEN** that change instance's spec commit state is `Untracked`

#### Scenario: Edited committed spec

- **WHEN** a change directory is tracked and at least one of its files has staged
  or unstaged modifications
- **THEN** that change instance's spec commit state is `Modified`

#### Scenario: Mixed tracked edit and new file resolves to Modified

- **WHEN** a change directory has both a tracked-but-modified file and a new
  untracked file
- **THEN** that change instance's spec commit state is `Modified`

#### Scenario: Committed and clean spec

- **WHEN** a change directory is fully tracked with no uncommitted edits
- **THEN** that change instance's spec commit state is `Committed`

### Requirement: Per-Repository Dirty Rollup

Each git-backed repository view SHALL expose a rollup of its worktrees' status: a
`dirty` flag that is true when any worktree has any uncommitted change, the set of
worktree paths that are dirty, and a `has_uncommitted_specs` flag that is true
when any change instance in the repository has a spec commit state other than
`Committed`. Non-git (flat) workspaces SHALL NOT carry these fields.

#### Scenario: Dirt from a non-spec file still marks the repo dirty

- **WHEN** a worktree has an uncommitted change only in files outside
  `openspec/`
- **THEN** the repository's `dirty` flag is true
- **AND** `has_uncommitted_specs` is false

#### Scenario: Uncommitted spec sets both rollups

- **WHEN** a worktree has an untracked or modified change directory
- **THEN** the repository's `dirty` flag is true
- **AND** `has_uncommitted_specs` is true
- **AND** the dirty worktree's path is included in the dirty-worktrees set

#### Scenario: Clean repository carries no dirt

- **WHEN** every worktree of a repository is clean
- **THEN** the repository's `dirty` flag is false
- **AND** `has_uncommitted_specs` is false
- **AND** the dirty-worktrees set is empty

### Requirement: Status Freshness

The whole-repository dirty rollup SHALL refresh in response to git index changes
and to the main window regaining focus, in addition to the existing
`openspec/`-scoped file-change and refs-change events. The application SHALL NOT
watch the entire working tree and SHALL NOT require a background poll for this
signal. A purely-unstaged change to a non-spec file MAY leave the rollup stale
until the next window focus or git event.

#### Scenario: Staging a change refreshes the rollup

- **WHEN** the user stages or commits a previously-uncommitted change in a
  tracked worktree
- **THEN** within the monitor's debounce window the repository's dirty rollup
  reflects the new state

#### Scenario: Focus refreshes after an external edit

- **WHEN** a non-spec file is modified while the main window is unfocused
- **AND** the main window regains focus
- **THEN** the repository's dirty rollup is recomputed
