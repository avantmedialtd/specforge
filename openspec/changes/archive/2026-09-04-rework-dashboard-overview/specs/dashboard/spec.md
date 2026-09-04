# dashboard

## ADDED Requirements

### Requirement: Dashboard Section Order

The Dashboard SHALL present its sections in a fixed vertical order: the
developer profile and streak, today's progress counts, today's ships feed, the
contribution heatmap, the per-author leaderboard, then the analytics band. That
order SHALL NOT depend on whether a section is in its quiet state, so no section
changes position as the day's activity accumulates.

The terminal frontend SHALL present the sections it renders in the same relative
order, so the two frontends do not disagree about what the Dashboard leads with.

#### Scenario: Today's ships precedes the heatmap

- **WHEN** the Dashboard renders
- **THEN** the today's ships feed appears above the contribution heatmap
- **AND** it appears below today's progress counts

#### Scenario: A quiet section does not move its neighbours

- **WHEN** no change has been archived on the viewer's local today
- **THEN** the today's ships feed still occupies its position between today's
  progress counts and the contribution heatmap
- **AND** no section below it moves up the page

#### Scenario: The terminal frontend agrees on the order

- **WHEN** the terminal frontend renders its dashboard screen
- **THEN** its ships-today section appears above its activity section

### Requirement: Analytics Band Composition

The Dashboard SHALL group its cross-workspace analytics into a single labelled
band containing the per-repository breakdown. The band's label row SHALL carry
the change-lifecycle figures — the windowed archive throughput and the average
time-to-archive — so those figures are presented without a card of their own.

The band SHALL NOT present a commits-per-day chart.

#### Scenario: Lifecycle figures accompany the band label

- **WHEN** the Dashboard renders its analytics band
- **THEN** the band's label row shows the number of changes archived within the
  window, the window's length, and the average time-to-archive
- **AND** those figures are not presented as a separate card

#### Scenario: No commits-per-day chart is shown

- **WHEN** the Dashboard renders its analytics band
- **THEN** no chart of commits per calendar day is present

## MODIFIED Requirements

### Requirement: Per-Repository Breakdown

The Dashboard SHALL present a breakdown of top-level registered items — a
repository group or a non-git (flat) workspace — showing each presented item's
count of active changes and its count of archived changes. Each entry SHALL be
labelled with the same display name the tree pane uses for that top-level row.

Entries SHALL be ordered by active-change count descending, then by
archived-change count descending, then by label ascending. All three keys are
required, so that entries with equal counts hold a stable position across
refreshes rather than trading places.

The breakdown SHALL present at most a fixed maximum number of entries,
independent of how many top-level items are registered, so that the breakdown's
height does not vary with the size of the registry. That maximum is

$$N = 5.$$

When entries are withheld, the breakdown SHALL present a remainder line stating
how many were withheld and, when any withheld entry has at least one active
change, how many withheld entries have active changes. The remainder line SHALL
NOT restate the registry-wide archived total, which the Dashboard's summary line
already carries (see the *Cross-Workspace Summary Metrics* requirement).

An entry with at least one active change SHALL present a proportional bar whose
length encodes that entry's active-change count relative to the largest
active-change count among the presented entries. An entry with no active changes
SHALL NOT present a bar, and SHALL be visually de-emphasised relative to the
entries that have one, so that every bar rendered encodes a non-zero quantity
and the visual ordering agrees with the sort order.

A bar's length SHALL encode a count and SHALL NOT grow without bound with the
pane's width. Widening the pane SHALL widen the breakdown's container, per the
*Dashboard Fills Available Width* requirement, without lengthening a bar that
represents an unchanged count.

Withholding entries is a presentation concern. The Dashboard's underlying
cross-workspace data SHALL retain every top-level item, so that totals derived
from it — including the registry-wide archived count in the summary line —
remain complete regardless of how many entries the breakdown presents.

#### Scenario: Entries are ordered by active changes

- **WHEN** the Dashboard renders with one repository holding two active changes
  and another holding one
- **THEN** the repository with two active changes is presented above the
  repository with one

#### Scenario: Ties are broken by archived count, then by label

- **WHEN** two repositories hold the same number of active changes
- **THEN** the one with more archived changes is presented first
- **AND** when their archived counts are also equal, they are presented in
  ascending label order
- **AND** their relative order is unchanged by a subsequent Dashboard refresh
  that does not change either count

#### Scenario: The breakdown is capped and states what it withheld

- **WHEN** more top-level items are registered than the breakdown presents
- **THEN** the breakdown presents no more than the fixed maximum number of
  entries
- **AND** a remainder line states how many entries were withheld

#### Scenario: A withheld entry with active changes is reported

- **WHEN** entries are withheld and at least one of them has an active change
- **THEN** the remainder line states how many withheld entries have active
  changes

#### Scenario: An entry with active changes shows a proportional bar

- **WHEN** a presented entry has at least one active change
- **THEN** it shows a bar whose length is proportional to its active-change
  count relative to the largest active-change count among the presented entries

#### Scenario: An entry with no active changes shows no bar

- **WHEN** a presented entry has no active changes
- **THEN** it shows no bar
- **AND** it is visually de-emphasised relative to entries that show one

#### Scenario: Bar length does not follow the pane width

- **WHEN** the pane is widened and no entry's counts have changed
- **THEN** the breakdown's container widens
- **AND** no entry's bar becomes longer

#### Scenario: Breakdown labels match the tree

- **WHEN** a repository group has a configured display name
- **THEN** its breakdown entry is labelled with that same display name

#### Scenario: Capping does not reduce the registry-wide totals

- **WHEN** the breakdown withholds entries
- **THEN** the Dashboard's summary line still reports the archived count
  aggregated across every registered workspace, including the withheld ones

#### Scenario: A registry smaller than the cap presents every entry

- **WHEN** fewer top-level items are registered than the fixed maximum
- **THEN** every one of them is presented
- **AND** no remainder line is shown

### Requirement: Change Lifecycle Metrics

The Dashboard SHALL present change-lifecycle metrics derived from git history:
the throughput (the number of changes archived within a recent bounded window)
and the average time-to-archive (the mean elapsed time between a change's
creation and its archival). A change's creation date SHALL be the date of the
earliest commit that added a file under its `openspec/changes/<id>/` directory;
its archive date SHALL be the date of the earliest commit that added a file
under `openspec/changes/archive/<id>/`. Only changes for which both dates are
recoverable from git SHALL contribute to the average time-to-archive.

The throughput window SHALL be bounded and SHALL be named wherever the
throughput figure is presented, so the figure is legible without reference to
any other surface. The Dashboard SHALL present the window's length alongside the
figures rather than relying on a neighbouring card's caption to supply it.

Lifecycle data is derived from append-only history — once a change's creation
and archival commits exist, their instants do not change — so the application
SHALL derive a repository's lifecycle data **at most once per change to that
repository's history**, rather than once per Dashboard fetch. A Dashboard fetch
whose repositories' histories have not moved since the previous fetch SHALL
issue no lifecycle-mining git invocation.

When a repository's history moves, the application SHALL re-derive that
repository's lifecycle data and SHALL NOT re-derive any other repository's. The
Dashboard SHALL reflect the moved history within the watcher's debounce window,
so the freshness the *Reactive Dashboard Updates* requirement guarantees is
unchanged.

Concurrent derivations for the same repository SHALL be collapsed into a single
mining invocation, so that overlapping fetches cannot each issue their own.

The metrics SHALL be identical to those a per-fetch derivation would produce for
the same history.

#### Scenario: Throughput counts recent archives

- **WHEN** three changes were archived within the window and others were
  archived earlier
- **THEN** the throughput metric reports the three changes archived within the
  window

#### Scenario: The window's length is presented with the figures

- **WHEN** the Dashboard presents the lifecycle figures
- **THEN** the length of the throughput window is presented alongside them

#### Scenario: Average time-to-archive uses recoverable lifecycles

- **WHEN** a set of changes has both a creation commit and an archive commit
  recoverable from git
- **THEN** the average time-to-archive is the mean of each such change's
  archive-date-minus-creation-date
- **AND** changes whose creation or archive date cannot be recovered from git
  are excluded from the average

#### Scenario: No recoverable lifecycles

- **WHEN** no change has both a recoverable creation and archive date
- **THEN** the average time-to-archive renders as unavailable rather than as an
  error or a zero average

#### Scenario: An unchanged repository is not re-mined

- **WHEN** the Dashboard is fetched
- **AND** no registered repository's history has moved since the previous fetch
- **THEN** no lifecycle-mining git invocation is issued

#### Scenario: A commit re-mines only its own repository

- **WHEN** a commit lands in one registered repository
- **AND** the Dashboard is fetched
- **THEN** that repository's lifecycle data is re-derived
- **AND** no other repository's lifecycle data is re-derived

#### Scenario: Concurrent fetches mine once

- **WHEN** two Dashboard fetches overlap for a repository whose history has
  moved
- **THEN** that repository's lifecycle data is mined once
- **AND** both fetches reflect the same derived metrics

### Requirement: Graceful Degradation Without Git

When the `git` binary is unavailable, or a registered workspace is not inside a
git repository, the Dashboard SHALL still render its non-git sections (summary
metrics, per-repository breakdown, today's ships feed) and SHALL render the
git-derived lifecycle metrics using only the data recoverable from the available
git-backed repositories. The today's ships feed's membership is determined from
the dated archive directory and SHALL render without git; only its per-entry
relative archive time, which is git-derived, SHALL be omitted when git is
unavailable. The Dashboard SHALL NOT error when git is absent.

A **failed** lifecycle derivation SHALL NOT be retained as though it were a
result. Because a repository with no changes and a repository whose mining
failed both yield no lifecycle data, the application SHALL distinguish the two
and retain only successful derivations; a failed derivation SHALL be retried on
a subsequent fetch rather than serving an empty lifecycle for the remainder of
the session. A repository that genuinely has no changes is a successful
derivation and MAY be retained.

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** the Dashboard renders its summary metrics, per-repository breakdown,
  and today's ships feed
- **AND** the today's ships entries render without their relative archive times
- **AND** the lifecycle metrics render an empty or unavailable state rather than
  erroring

#### Scenario: Mixed git and non-git workspaces

- **WHEN** some registered workspaces are git-backed and others are flat
- **THEN** the summary metrics and breakdown include every workspace
- **AND** the lifecycle metrics include only the git-backed repositories

#### Scenario: A transient mining failure is retried

- **WHEN** a repository's lifecycle derivation fails
- **AND** the Dashboard is fetched again
- **THEN** the derivation is retried for that repository
- **AND** a subsequent successful derivation is reflected in the lifecycle
  metrics

#### Scenario: A repository with no changes is retained

- **WHEN** a repository's lifecycle derivation succeeds and finds no changes
- **AND** the Dashboard is fetched again with that repository's history unmoved
- **THEN** no further lifecycle-mining invocation is issued for that repository

### Requirement: Dashboard Includes Disabled Workspaces

Disabling a top-level row (see the *Workspace Disable State* requirement in the
`workspace-registry` capability) SHALL have no effect on any Dashboard surface.
A disabled workspace SHALL continue to contribute to the cross-workspace summary
metrics, the per-repository breakdown, the change lifecycle metrics, today's
ships feed, the today's-progress hero, and the streak and contribution heatmap.

This asymmetry is deliberate. Disabling is an attention control, not an
existence control: it silences the tree pane, the tray badge, and desktop
notifications, while the Dashboard remains the unfiltered record of what the
user has registered and accomplished. It follows that the Dashboard's
active-change total will exceed the number of changes reachable through the tree
pane whenever any workspace is disabled, and the Dashboard SHALL note that its
totals include disabled workspaces so the discrepancy is legible rather than
surprising. That note SHALL count disabled **top-level rows** — the rows the
tree actually drops — and not registered folders: the disabled flag is stored
per row, so a repository the user registered at several worktrees has several
registered folders carrying it while the tree loses exactly one row.

Because the Dashboard reads only cache-derived fields from the aggregated view —
active and archived logical changes, task rollups, and capability-spec counts —
and never the git-derived working-tree fields, a disabled row's omitted git
state SHALL NOT degrade any Dashboard figure.

The per-repository breakdown presents a bounded number of entries (see the
*Per-Repository Breakdown* requirement). A disabled workspace SHALL be ranked by
exactly the same keys as an enabled one, so that its disabled state never
changes its position and never determines whether it is presented. Being
withheld by the cap is therefore a consequence of its counts, never of its
disabled state.

#### Scenario: Summary metrics include disabled workspaces

- **WHEN** two workspaces are registered, one enabled with five active changes
  and one disabled with four
- **THEN** the Dashboard's active-change summary reports nine
- **AND** the tree pane shows only the enabled workspace's five

#### Scenario: Disabling a workspace does not change its rank

- **WHEN** a registered repository is disabled
- **THEN** its position in the Dashboard's breakdown ordering is unchanged
- **AND** it shows its active-change and archived-change counts wherever it is
  presented
- **AND** it is labelled with the same display name it had before being disabled

#### Scenario: A disabled workspace is withheld only by its counts

- **WHEN** a disabled repository holds more active changes than every enabled one
- **THEN** it is presented first in the breakdown
- **AND** it is not withheld in favour of an enabled repository with fewer
  active changes

#### Scenario: Lifecycle metrics include disabled repositories

- **WHEN** a disabled repository's changes were archived within the window
- **THEN** those changes contribute to the lifecycle throughput metrics
- **AND** they contribute to the average time-to-archive on the same terms as an
  enabled repository's

#### Scenario: Ships from a disabled workspace still appear

- **WHEN** a change in a disabled workspace is archived today
- **THEN** it appears in today's ships feed
- **AND** the entry is marked as belonging to a disabled workspace
- **AND** selecting it leads to the settings view where the workspace can be
  re-enabled, rather than doing nothing (see the *Ship Selection Opens the
  Archive Browser* requirement)

#### Scenario: The disabled-workspace note counts rows, not registered folders

- **WHEN** one repository is registered at two worktrees and is disabled
- **THEN** the Dashboard's note reports one disabled workspace
- **AND** the tree pane has dropped exactly one top-level row

#### Scenario: Streak and heatmap are unaffected

- **WHEN** a workspace is disabled for a period during which the user completes
  tasks and archives changes in it
- **THEN** those days count toward the streak and the contribution heatmap
- **AND** no streak day is lost as a result of the workspace having been
  disabled

#### Scenario: Dashboard renders when every workspace is disabled

- **WHEN** every registered workspace is disabled
- **THEN** the Dashboard renders without error
- **AND** its summary metrics, breakdown, and lifecycle metrics still reflect
  all registered workspaces
- **AND** the tray badge is hidden and the tree pane is empty

### Requirement: Reactive Dashboard Updates

While the Dashboard is the active center-pane surface, it SHALL reflect on-disk changes within the watcher's debounce window without user action. After the watcher finishes processing a debounced batch — a change added, a change archived, content edited within a tracked change, or a repository's refs changing — the Dashboard SHALL refresh its metrics to observe the post-batch state.

A single debounced batch SHALL cause **at most one** Dashboard refresh, however many distinct cache events that batch emits. The backend deliberately emits several events per batch (for example an archival emits a change-archived event, a generic update, and the derived logical/instance diff events), and the Dashboard subscribes to more than one of them; the Dashboard SHALL coalesce all events observed within the same event-loop turn into a single refetch rather than refetching per event.

While a refresh is in flight, a further event SHALL NOT start a second concurrent refetch; it SHALL instead cause exactly one follow-up refresh after the in-flight one settles, so that overlapping batches cannot accumulate outstanding requests.

#### Scenario: Dashboard updates when a change is added

- **WHEN** the Dashboard is the active surface
- **AND** a new change directory is created on disk in a registered workspace
- **THEN** the Dashboard's active-change count reflects the new change within the debounce window

#### Scenario: Dashboard updates when a change is archived

- **WHEN** the Dashboard is the active surface
- **AND** a change is moved to `openspec/changes/archive/` on disk
- **THEN** the Dashboard's active/archived counts and lifecycle metrics reflect the archival within the debounce window
- **AND** when the archive directory is dated the viewer's local today, the today's ships feed reflects it within the debounce window

#### Scenario: Dashboard updates on commit activity

- **WHEN** the Dashboard is the active surface
- **AND** a new commit is created in a registered git-backed repository
- **THEN** the Dashboard's commit-derived surfaces — the contribution heatmap and the day's commit count — reflect the new commit within the debounce window

#### Scenario: A multi-event batch refreshes the Dashboard once

- **WHEN** the Dashboard is the active surface
- **AND** a single debounced batch emits a change-archived event, a generic update event, and a derived logical-change event
- **THEN** the Dashboard issues exactly one refresh request for that batch

#### Scenario: Overlapping batches do not stack requests

- **WHEN** a Dashboard refresh is in flight
- **AND** a further batch emits cache events before it settles
- **THEN** no second concurrent refresh request is issued
- **AND** exactly one follow-up refresh runs after the in-flight one settles

## REMOVED Requirements

### Requirement: Git-Mined Activity Chart

**Reason**: the chart re-answered, at fourteen days and four buckets of vertical
resolution, a question the contribution heatmap answers over a year — and it did
so in a card whose height was dictated by the breakdown beside it rather than by
its own content. The terminal frontend has never rendered it. Its only unique
contribution was that its commit dates were not filtered through `is_me`, so it
alone showed every author's daily volume; that signal is given up deliberately,
with the per-author leaderboard retaining year-long commit totals per author and
the commit garden retaining today's per-author, per-repository detail.

The requirement also owned the definition of the bounded window that the *Change
Lifecycle Metrics* requirement referred to without restating. That definition
moves into *Change Lifecycle Metrics*, which now also requires the window's
length to be presented alongside the figures it bounds.
