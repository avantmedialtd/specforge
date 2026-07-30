# working-tree-status Specification

## Purpose

Defines how the application surfaces git working-tree state for registered repositories: a per-change spec commit state, a per-repository dirty rollup, and the freshness contract that keeps them current. The state is derived from a single `git status --porcelain` per worktree and degrades gracefully when git is unavailable.

## Requirements

### Requirement: Per-Worktree Working-Tree Status

For every tracked git worktree, the application SHALL compute the worktree's
git working-tree status with a single porcelain invocation
(`git status --porcelain --untracked-files=all`) issued through the application's
git command chokepoint, so that the WSL backend is used transparently where it
applies. The computation SHALL yield, from that one invocation, both a
whole-worktree dirty bit and a per-change classification, and SHALL degrade to a
clean/unknown result (no dirty bit, no per-change state) when git is unavailable
or the command fails.

The status invocation SHALL be read-only with respect to the repository: it MUST
NOT cause git to write the index or any other tracked git state (it runs with
`--no-optional-locks` so git does not opportunistically refresh the index stat
cache). This is required because the index is itself watched to keep the status
fresh; a status check that wrote the index would re-trigger that watcher and
loop.

#### Scenario: Clean worktree

- **WHEN** a worktree has no staged, unstaged, or untracked changes
- **THEN** its computed status is not dirty
- **AND** every change in that worktree classifies as committed

#### Scenario: Status survives a missing git binary

- **WHEN** the status computation cannot run git for a worktree
- **THEN** the worktree is reported as not dirty with no per-change state
- **AND** no error is surfaced to the user for that worktree

#### Scenario: Status check does not write the index

- **WHEN** a worktree's status is computed while its index stat cache is stale
  (a tracked file's mtime changed but its content did not)
- **THEN** the computation does not rewrite `.git/index`
- **AND** therefore does not re-trigger the index watcher that drives it

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

An event observed for a single repository SHALL recompute the working-tree status
of only that repository's worktrees and merge the result into the cached
aggregated view; it SHALL NOT issue a `git status` invocation for the worktrees of
any other registered repository. This bound applies equally to **git events**
(index, refs, `HEAD`) and to **`openspec/`-scoped file-change events** — an edit
to a change artifact in one repository's worktree SHALL NOT sweep the worktrees of
any other registered repository. A file-change event in a workspace with no
resolvable repository (a flat, non-git workspace) has no originating repository to
bound to and MAY perform a full recompute.

The window-focus refresh remains a full recompute across all repositories. The
freshness timing above is unchanged — only the work performed per event is bounded
to the originating repository, and the merged result SHALL be identical to what a
full recompute would have produced for that repository.

#### Scenario: Staging a change refreshes the rollup

- **WHEN** the user stages or commits a previously-uncommitted change in a
  tracked worktree
- **THEN** within the monitor's debounce window the repository's dirty rollup
  reflects the new state

#### Scenario: Focus refreshes after an external edit

- **WHEN** a non-spec file is modified while the main window is unfocused
- **AND** the main window regains focus
- **THEN** the repository's dirty rollup is recomputed

#### Scenario: A git event in one repository does not recompute others

- **WHEN** a git index or refs change is observed for repository A
- **AND** repositories B and C are also registered
- **THEN** only repository A's worktree status is recomputed
- **AND** no `git status` invocation is issued for B's or C's worktrees
- **AND** the resulting rollup for A is identical to a full recompute of A

#### Scenario: A spec file edit in one repository does not recompute others

- **WHEN** a file under `openspec/changes/` is edited in a worktree of repository A
- **AND** repositories B and C are also registered
- **THEN** only repository A's worktree status is recomputed
- **AND** no `git status` invocation is issued for B's or C's worktrees
- **AND** the resulting aggregated view for A is identical to a full recompute of A

#### Scenario: A file edit in a flat workspace still refreshes

- **WHEN** a file under `openspec/changes/` is edited in a registered non-git
  workspace
- **THEN** the aggregated view reflects the edit within the debounce window

### Requirement: Non-Blocking Aggregated Recompute

The aggregated recompute SHALL NOT hold the workspace registry or the parsed-state
cache locked while git subprocesses run. Lock acquisition is permitted to gather
the recompute's inputs and to merge its result, but the git invocations themselves
SHALL execute with no such lock held.

Consequently, a concurrent reader or writer of the cache — another workspace's
watcher, or an IPC command handler — SHALL NOT be blocked for the duration of a
recompute's git I/O.

The recompute SHALL NOT run on an async runtime worker thread; it executes on a
blocking pool so that the runtime remains available to service events and commands
while git subprocesses are outstanding.

#### Scenario: A command handler is not blocked by an in-flight recompute

- **WHEN** an aggregated recompute is in flight, with its git invocations outstanding
- **AND** an IPC command that reads the parsed-state cache is issued
- **THEN** the command observes the cache without waiting for the recompute's git
  invocations to complete

#### Scenario: A second workspace's watcher is not blocked

- **WHEN** an aggregated recompute for repository A is in flight
- **AND** a file-change batch for a workspace of repository B is processed
- **THEN** B's cache update is not blocked for the duration of A's git invocations

### Requirement: Concurrent Per-Worktree Status Invocation

The per-worktree git invocations of a recompute MAY be issued concurrently, since
each worktree's status is independent of every other's. The number of concurrently
outstanding git subprocesses SHALL be bounded, so that a registry with many
worktrees cannot fan out to an unbounded number of processes.

The result SHALL be independent of completion order: the aggregated view produced
by a concurrent recompute SHALL be identical to the one a serial recompute would
produce over the same inputs, including the ordering of worktrees within a
repository.

#### Scenario: Concurrent recompute matches the serial result

- **WHEN** an aggregated view is computed concurrently over a registry
- **AND** the same aggregated view is computed serially over the same inputs
- **THEN** the two results are identical, worktree ordering included

#### Scenario: Worktree fan-out stays bounded

- **WHEN** a recompute runs over a registry with more worktrees than the
  concurrency bound
- **THEN** the number of simultaneously outstanding git subprocesses does not
  exceed that bound
