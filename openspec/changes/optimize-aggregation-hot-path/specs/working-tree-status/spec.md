## MODIFIED Requirements

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

## ADDED Requirements

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
