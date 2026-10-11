## ADDED Requirements

### Requirement: Archived Instances Are Ordered Without Reading the Archive

The watcher's aggregation SHALL NOT read, list, or stat anything beneath an
archived change's own directory (`openspec/changes/archive/<dir>/`). Its access to
the archive SHALL be limited to the listing of `openspec/changes/archive/` itself
that the *On-Demand, Off-Hot-Path Loading* requirement permits, so the cost of a
recompute grows with the number of archive directories and never with the number
of files inside them.

Where the aggregation orders the archived instances of one logical change — the
copies of that change archived in different worktrees of a repository — it SHALL
derive each instance's recency from the `YYYY-MM-DD` prefix of its archive
directory name, the same date the Archive view displays (see *Newest-First Ordering
and Date Labels*). A copy with a later date SHALL order before a copy with an
earlier one. An undated legacy directory SHALL order after every dated copy.
Copies sharing a date SHALL keep the order of their worktrees within the
repository, which is the configured order of that repository's tracked
worktrees, so the result is deterministic across recomputes of unchanged
content. Any surface that picks "the" copy of an archived change from that order,
such as the Dashboard's list of changes archived today, SHALL therefore pick the
same copy on every recompute.

The ordering of a logical change's **active** instances SHALL NOT depend on its
archived instances.

#### Scenario: A recompute does not descend into archived changes

- **WHEN** a file under a registered workspace's active `openspec/changes/` tree
  is modified, triggering a watcher re-aggregation
- **AND** the workspace's archive holds many archived changes, each with nested
  `specs/` directories
- **THEN** no directory beneath any `openspec/changes/archive/<dir>/` is listed
  and no file beneath one is stat-ed or read by the re-aggregation

#### Scenario: File timestamps inside an archived change do not affect ordering

- **WHEN** a logical change is archived as `2026-06-04-foo` in worktree W1 and as
  `2026-06-05-foo` in worktree W2
- **AND** a file inside W1's copy has a modification time later than every file in
  W2's copy
- **THEN** W2's archived instance orders before W1's

#### Scenario: Same-day copies keep worktree order

- **WHEN** a logical change is archived on the same date in worktrees W1 and W2,
  with W1 ahead of W2 in the repository's worktree order
- **THEN** W1's archived instance orders before W2's on every recompute
- **AND** the copy the Dashboard reports as archived today is W1's on every
  recompute
