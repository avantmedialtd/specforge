# workspace-registry

## ADDED Requirements

### Requirement: Bounded Repository Watcher Footprint

The filesystem-watching infrastructure SHALL maintain at most one native
filesystem watcher per registered repository for all repository-level git
signals — worktree-set changes, default-branch changes, refs/commit-graph
changes, and working-tree index changes — rather than a separate watcher per
signal. Installing a repository's watcher SHALL be idempotent: a repository that
is already watched MUST NOT gain an additional watcher when it is re-registered or
re-discovered. When the last tracked workspace belonging to a repository is
removed, that repository's watcher SHALL be disposed.

#### Scenario: One watcher per repository

- **WHEN** a repository with any number of tracked worktrees is being watched
- **THEN** the application holds exactly one repository-level filesystem watcher
  for that repository's git signals

#### Scenario: Watcher count stays bounded over a long session

- **WHEN** repositories are registered, re-discovered, and have their worktrees
  added and removed repeatedly over a long-running session
- **THEN** the number of repository-level watchers equals the number of distinct
  registered repositories
- **AND** it does not accumulate beyond that count

#### Scenario: Removing a repository disposes its watcher

- **WHEN** the last tracked workspace belonging to a repository is removed from
  the registry
- **THEN** that repository's filesystem watcher is disposed

### Requirement: Coalesced Repository Refresh

A burst of git filesystem events for a single repository SHALL be coalesced into
at most one working-tree status recompute and at most one commit-graph refresh per
debounce window, rather than one recompute per raw filesystem event. Such bursts
include the many index and ref writes produced by a rebase, fetch, or checkout.

#### Scenario: A multi-write git operation triggers one refresh

- **WHEN** a git operation writes many index and ref entries for a repository
  within a single debounce window
- **THEN** the repository's working-tree status is recomputed at most once for
  that window
- **AND** its commit-graph is refreshed at most once for that window
