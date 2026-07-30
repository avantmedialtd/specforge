## MODIFIED Requirements

### Requirement: Coalesced Repository Refresh

A burst of git filesystem events for a single repository SHALL be coalesced into
at most one working-tree status recompute and at most one commit-graph refresh per
debounce window, rather than one recompute per raw filesystem event. Such bursts
include the many index and ref writes produced by a rebase, fetch, or checkout.

The coalescing bound SHALL hold across the *contents* of a batch as well as its
raw events: when a single debounced batch adds or removes multiple tracked
worktrees, the application SHALL perform at most one aggregated recompute for that
batch, rather than one per added or removed worktree. Watcher installation and
teardown for the affected worktrees happen first; the single recompute and its
derived events follow, and observe the post-batch set of worktrees.

The `WorkspaceRemoved` and `Updated` events announcing individual worktree
additions and removals SHALL still be emitted per affected worktree — only the
aggregated recompute is coalesced — and the derived logical/instance diff events
SHALL be emitted after them, computed once against the post-batch state.

#### Scenario: A multi-write git operation triggers one refresh

- **WHEN** a git operation writes many index and ref entries for a repository
  within a single debounce window
- **THEN** the repository's working-tree status is recomputed at most once for
  that window
- **AND** its commit-graph is refreshed at most once for that window

#### Scenario: Adding several worktrees at once triggers one recompute

- **WHEN** a single debounced batch adds three worktrees to a tracked repository
- **THEN** the aggregated view is recomputed exactly once for that batch
- **AND** the recomputed view includes all three new worktrees

#### Scenario: Removing several worktrees at once triggers one recompute

- **WHEN** a single debounced batch removes two worktrees from a tracked repository
- **THEN** the aggregated view is recomputed exactly once for that batch
- **AND** a `WorkspaceRemoved` event is emitted for each removed worktree
