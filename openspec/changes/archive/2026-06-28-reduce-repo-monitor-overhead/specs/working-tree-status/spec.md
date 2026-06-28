# working-tree-status

## MODIFIED Requirements

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

### Requirement: Status Freshness

The whole-repository dirty rollup SHALL refresh in response to git index changes
and to the main window regaining focus, in addition to the existing
`openspec/`-scoped file-change and refs-change events. The application SHALL NOT
watch the entire working tree and SHALL NOT require a background poll for this
signal. A purely-unstaged change to a non-spec file MAY leave the rollup stale
until the next window focus or git event.

A git event observed for a single repository SHALL recompute the working-tree
status of only that repository's worktrees and merge the result into the cached
aggregated view; it SHALL NOT issue a `git status` invocation for the worktrees of
any other registered repository. The window-focus refresh remains a full recompute
across all repositories. The freshness timing above is unchanged — only the work
performed per event is bounded to the originating repository, and the merged
result SHALL be identical to what a full recompute would have produced for that
repository.

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
