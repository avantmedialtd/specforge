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
any other registered repository.

The two kinds of event are bounded differently *within* the repository:

- A **git event** (index, `HEAD`, local branch refs under `refs/heads/` or
  `packed-refs`, or repository configuration) SHALL
  recompute the working-tree status, branch and upstream of **every** tracked
  worktree of that repository, because such an event can change any of them.
- An **`openspec/`-scoped file-change event** observed in one worktree SHALL
  recompute the working-tree status of **only that worktree**. Every other warm
  worktree of the same repository SHALL reuse the branch, upstream and status it
  carried at its most recent status computation, provided that computation
  classified the same set of active change identifiers the worktree holds now
  and the repository's git events are observed. A repository whose git events
  cannot be observed (a WSL repository, or one whose git-directory watcher
  could not be established) SHALL re-status every worktree on such an event,
  because no git event would ever correct a reused value.
  A worktree with no such prior computation — first appearance, a set of active
  changes that has since changed, or a row last aggregated cold — SHALL be
  statused as part of the same recompute. A file edit inside one worktree's
  `openspec/` tree cannot change another worktree's branch, upstream, or spec
  commit states; the only state of a sibling worktree such reuse can leave stale is
  a purely-unstaged change to a non-spec file, which the tolerance above already
  admits.

A file-change event in a workspace with no resolvable repository (a flat,
non-git workspace) has no originating repository to bound to. It SHALL rebuild
only that workspace's own top-level row from the parsed-state cache and merge it
into the cached aggregated view at that row's config position (see the
*Deterministic Config-Ordered Top-Level View* requirement in the
`workspace-registry` capability), re-deriving its OpenSpec presence. It SHALL NOT
spawn any git process — no `git status`, `git worktree list`, or `git config` —
for any registered repository or for the flat workspace itself. The one
exception is a workspace whose row is not yet in the cached aggregated view, or
that is no longer a registered flat workspace: that event SHALL be handled by a
full recompute, as a repository's first appearance is.

The window-focus refresh remains a full recompute across all repositories. The
freshness timing above is unchanged — only the work performed per event is bounded
to the originating repository (and, for a file-change event, to the originating
worktree), and the merged result for the originating repository SHALL be identical
to what a full recompute would have produced for it whenever its other worktrees
carry no unsignalled change since their last status computation.

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
- **AND** no other worktree of A has changed since its last status computation
- **THEN** only repository A's worktree status is recomputed
- **AND** no `git status` invocation is issued for B's or C's worktrees
- **AND** the resulting aggregated view for A is identical to a full recompute of A

#### Scenario: A file edit in a flat workspace still refreshes

- **WHEN** a file under `openspec/changes/` is edited in a registered non-git
  workspace
- **THEN** the aggregated view reflects the edit within the debounce window

#### Scenario: A file edit in a flat workspace spawns no git process

- **WHEN** a file under `openspec/changes/` is edited in a registered non-git
  workspace
- **AND** git repositories A and B are also registered
- **THEN** no git subprocess of any kind is spawned while the batch is handled
- **AND** every other top-level row of the aggregated view is unchanged and keeps
  its position

#### Scenario: A spec file edit re-statuses only its own worktree

- **WHEN** repository A tracks worktrees W1 and W2, both previously statused
- **AND** a file under W1's `openspec/changes/` is edited
- **THEN** exactly one `git status` invocation is issued, anchored at W1
- **AND** no `git status` invocation is issued for W2
- **AND** W2's branch, dirty bit and per-change spec commit states in the
  aggregated view are the values from its last status computation

#### Scenario: A sibling worktree with a changed change set is re-statused

- **WHEN** repository A tracks worktrees W1 and W2
- **AND** W2's set of active changes differs from the set its last status
  computation classified
- **AND** a file under W1's `openspec/changes/` is edited
- **THEN** both W1 and W2 are statused in that recompute

#### Scenario: A git event still re-statuses every worktree of the repository

- **WHEN** repository A tracks worktrees W1 and W2
- **AND** a git index, refs or `HEAD` change is observed for A
- **THEN** a `git status` invocation is issued for both W1 and W2
