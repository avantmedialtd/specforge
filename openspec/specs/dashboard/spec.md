# dashboard Specification

## Purpose

Defines the Dashboard: the global, read-only overview rendered as the default home surface of the center pane. It aggregates state across every registered workspace — summary metrics, a per-repository breakdown, change-lifecycle throughput and time-to-archive, and a today's-ships feed — refreshing on the existing cache and graph events and degrading gracefully when git is unavailable.
## Requirements
### Requirement: Dashboard Home Surface

The application SHALL provide a Dashboard: a global, read-only overview rendered in the center (detail) pane. The Dashboard SHALL be the center pane's default render target — it SHALL be shown whenever the current address does not name another view, and whenever no artifact and no commit is selected, in place of any "nothing selected" placeholder.

At startup the Dashboard SHALL be shown when no address is supplied or the supplied address names the home surface. When an explicit address names another view, that view SHALL be rendered instead — see the *Cold-Load Address Resolution* requirement in the `view-routing` capability. An address that cannot be resolved SHALL be reported as not found rather than silently falling back to the Dashboard.

The tree pane SHALL render a pinned "Dashboard" entry at the top of the pane (mirroring the pinned Settings entry at the bottom). Selecting the Dashboard entry SHALL set the center pane to the Dashboard. Selecting a renderable artifact in the tree, or a commit in the rail, SHALL replace the Dashboard with that target; selecting the Dashboard entry again SHALL return the center pane to the Dashboard. The Dashboard entry SHALL convey an active treatment while the Dashboard is the current center-pane target.

#### Scenario: Dashboard shown at startup

- **WHEN** the user opens the main window with no address supplied and no artifact or commit has been selected
- **THEN** the center pane renders the Dashboard
- **AND** no "nothing selected" placeholder is shown

#### Scenario: An explicit address opens its view instead of the Dashboard

- **WHEN** the application is opened at an address naming a change artifact in a registered workspace
- **THEN** the center pane renders that artifact
- **AND** the Dashboard is not the center pane's target

#### Scenario: An unresolvable address does not silently fall back

- **WHEN** the application is opened at an address that cannot be resolved
- **THEN** the user is told the address could not be found
- **AND** the Dashboard is not rendered as though the address had named it

#### Scenario: Dashboard entry returns to the Dashboard

- **WHEN** the center pane is rendering an artifact or a commit detail
- **AND** the user selects the pinned Dashboard entry at the top of the tree
- **THEN** the center pane renders the Dashboard
- **AND** the Dashboard entry renders in its active state

#### Scenario: Selecting an artifact replaces the Dashboard

- **WHEN** the center pane is rendering the Dashboard
- **AND** the user selects a renderable artifact node in the tree
- **THEN** the center pane renders that artifact's markdown
- **AND** the Dashboard entry returns to its idle state

### Requirement: Cross-Workspace Summary Metrics

The Dashboard SHALL present, aggregated across every registered workspace, the total number of active (non-archived) changes — rendered as a compact summary line alongside the total archived count, not as a metric card. The Dashboard SHALL NOT present standalone Overview cards for the task rollup, for the count of active changes that touch a capability spec, or for the registered repository/worktree counts.

#### Scenario: Active-change summary reflects all workspaces

- **WHEN** the Dashboard renders with multiple registered workspaces
- **THEN** the active-change count equals the total number of non-archived changes across all of them

#### Scenario: No Overview summary cards

- **WHEN** the Dashboard renders its analytics
- **THEN** no card for the task rollup, the changes-touching-specs count, or the repository/worktree counts is shown

#### Scenario: Empty registry

- **WHEN** no workspaces are registered
- **THEN** the Dashboard renders without error
- **AND** the active-change summary shows a zero count

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

### Requirement: Today's Ships Feed

The Dashboard SHALL present a "Today's ships" feed: the changes archived today, aggregated across every registered workspace, ordered newest-archived first. A change SHALL be considered shipped today when its archived directory (`openspec/changes/archive/<YYYY-MM-DD>-<id>/`) is dated to the viewer's local calendar day, consistent with the day boundary used by the commit garden and the *Today's Progress* hero. The feed's membership SHALL be determined from the dated archive directory and SHALL NOT require git. Each feed entry SHALL identify its change — by its title when available, otherwise its change id — and its owning workspace or repository. When the change's archival instant is recoverable from git history, each entry SHALL additionally present a relative archive time (for example, "archived 2h ago"); when it is not recoverable, the entry SHALL render **neither the relative time nor the label that introduces it**, rather than an introduction with nothing after it. Entries SHALL be ordered by archival instant, newest first, falling back to a stable order when the instant is unavailable.

The relative archive time SHALL use the **same relative-time vocabulary** as every other surface in the application that presents an elapsed time — the workspace tree's per-instance modification time and the detail pane's change-identity header (see the `spec-browser` capability). One kind of value SHALL NOT be spelled differently on different surfaces, and this equivalence SHALL hold at every tier of the vocabulary, so that changing how one surface words an interval cannot leave the others behind. The vocabulary SHALL advance without user action wherever it is displayed, so a feed left open does not freeze at the moment it was painted.

#### Scenario: Feed lists changes archived today

- **WHEN** the today's ships feed renders
- **AND** one or more changes were archived to a directory dated the viewer's local today
- **THEN** those changes are listed, newest-archived first
- **AND** changes archived on an earlier day are not listed

#### Scenario: Entry shows a relative archive time when git supplies it

- **WHEN** a shipped change's archival instant is recoverable from git history
- **THEN** its feed entry shows a relative archive time

#### Scenario: An entry with no recoverable instant shows no archive-time text at all

- **WHEN** a shipped change's archival instant is not recoverable from git history
- **THEN** its feed entry renders no relative archive time
- **AND** it renders no introducing label left stranded without a time after it

#### Scenario: The feed words an interval as the rest of the application does

- **WHEN** the Dashboard's ships feed and any other surface presenting an elapsed time are compared
- **THEN** an interval of the same length is rendered in the same words on both
- **AND** this holds at every tier of the vocabulary

#### Scenario: Entry identifies change and workspace

- **WHEN** the today's ships feed renders an entry
- **THEN** the entry shows the change's title when available, otherwise its change id
- **AND** the entry shows the change's owning workspace or repository

### Requirement: Ship Selection Opens the Archive Browser

Selecting an entry in the today's ships feed SHALL open the Archive browser with that archived change pre-selected, rather than navigating to the active-change read path — an archived change no longer resides under `openspec/changes/<id>/`. This navigation SHALL be read-only, consistent with the Dashboard's read-only operation.

A feed entry SHALL be resolved to its owning top-level row by the repository it belongs to, not by the worktree path the change was archived from. A change is routinely archived from inside a feature worktree that hosts no active change afterwards, and such a worktree is neither the repository's main worktree nor any active change instance's path; resolving by worktree path alone would fail to open a perfectly reachable repository's ship.

Because the feed is deliberately unfiltered (see the *Dashboard Includes Disabled Workspaces* requirement), it SHALL also list ships whose top-level row is not present in the tree pane — a disabled row, or one that is no longer registered. Such an entry SHALL be visibly marked as such, and selecting it SHALL navigate to the settings view, where a disabled row is re-enabled and an unregistered one re-added. No feed entry SHALL be rendered as a control that does nothing when selected.

Selecting an entry SHALL NOT itself change any workspace's disabled state: parking is an explicit settings decision, and a navigation gesture never reverses it.

#### Scenario: Selecting a ship opens it in the Archive browser

- **WHEN** the user selects an entry in the today's ships feed
- **THEN** the Archive browser opens with that change pre-selected

#### Scenario: Selecting a ship archived from a worktree with no active change

- **WHEN** a change was archived inside a worktree that now hosts no active change
- **AND** its repository is present in the tree pane
- **THEN** selecting its feed entry opens the Archive browser for that repository with the change pre-selected

#### Scenario: Selecting a ship whose top-level row is disabled

- **WHEN** the user selects a feed entry whose owning repository is disabled
- **THEN** the settings view opens, where the workspace's toggle can be switched back on
- **AND** the workspace's disabled state is unchanged by the selection

#### Scenario: A ship whose top-level row is not in the tree is marked in the feed

- **WHEN** the today's ships feed renders an entry whose owning top-level row is disabled or no longer registered
- **THEN** the entry is marked as such alongside its workspace label
- **AND** the entry is still listed

#### Scenario: Ship selection performs no mutation

- **WHEN** the user selects an entry in the today's ships feed
- **THEN** the only effect is navigation — into the Archive browser, or into the settings view for a row that is not in the tree
- **AND** no spec, task, change, git state, or workspace disabled state is modified

### Requirement: Today's Ships Quiet State

The today's ships feed SHALL, when no change has been archived on the viewer's local today, present a quiet-day note rather than hiding the feed or showing stale prior-day entries.

#### Scenario: Nothing shipped yet today

- **WHEN** no change is archived to a directory dated the viewer's local today
- **THEN** the today's ships feed shows a quiet-day note
- **AND** it does not show changes archived on earlier days

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

### Requirement: Read-Only Operation

The Dashboard SHALL expose no operation that mutates a spec file, a repository, or any workspace state. Interacting with the Dashboard SHALL only navigate (select a change) or render metrics; it SHALL NOT edit specs, toggle task checkboxes, move changes, or run any git operation that changes history or working-tree state.

Recording observed achievements to the activity log SHALL persist only to the application's data directory and SHALL NOT write into any workspace's `openspec/` tree; this persistence does not constitute a mutation of workspace state.

#### Scenario: No mutating actions are offered

- **WHEN** the user interacts with the Dashboard
- **THEN** no action that edits a spec, toggles a task, archives a change, or mutates git state is available
- **AND** the only effect of selecting a Dashboard element is navigation to a change

#### Scenario: Activity recording does not mutate the workspace

- **WHEN** the Dashboard records an observed achievement
- **THEN** the record is written only to the application's data directory
- **AND** no file under any workspace's `openspec/` tree is created or modified

### Requirement: Dashboard Fills Available Width

The Dashboard SHALL fill the full available width of the center (detail) pane at any window size, rather than capping its content at a fixed maximum width or centering it within a narrower column. The Dashboard SHALL retain its surrounding padding. The widths and behaviour of the surrounding shell — the tree (sidebar) pane and the commit-graph rail — SHALL be unaffected; only the Dashboard's own content width follows the pane.

#### Scenario: Wide pane has no dead gutters

- **WHEN** the Dashboard renders in a center pane wider than its former cap
- **THEN** the Dashboard content extends to the full width of the pane (minus its padding)
- **AND** no centered fixed-width column with empty gutters on either side is shown

#### Scenario: Content reflows to fill

- **WHEN** the available pane width increases
- **THEN** the Dashboard's proportional panels and grids reflow to occupy the additional width
- **AND** no horizontal scrollbar is introduced by the Dashboard content

#### Scenario: Surrounding shell is unaffected

- **WHEN** the Dashboard is the active center-pane surface
- **THEN** the sidebar pane and the commit-graph rail retain their existing widths and behaviour

### Requirement: Today's Progress Hero

The Dashboard SHALL present a "Today's Progress" band as its topmost content, showing four headline counts aggregated across all registered workspaces. Three of the counts reflect achievements recorded for the current local calendar day — changes archived (shipped), commits landed, and tasks completed. The remaining count reflects the *current* number of active (non-archived) changes the developer has in flight — the changes the developer created, consistent with the personal resolution of the progress frame — which is a live state count rather than a today count. The counts SHALL be presented as a fixed left-to-right sequence: changes archived (shipped), changes in flight, commits landed, then tasks completed — so the two change-level counts lead as a pair and the within-change increments follow. Each count SHALL render with an animated count-up on first render. Each of the three today-flow counts (shipped, commits, tasks completed) SHALL be accompanied by a comparison to the user's recent daily average for that achievement type; the in-flight count SHALL NOT show an average comparison, as a live level has no trailing daily average. When the viewer's `prefers-reduced-motion` setting is active, counts SHALL render at their final value without animation. The day boundary for the today-flow counts SHALL be the viewer's local calendar day, consistent with the commit-graph rail's day grouping.

#### Scenario: Today's flow counts reflect the current day

- **WHEN** the Dashboard renders
- **THEN** each of the shipped, commits, and tasks-completed counts equals the number of achievements of that type recorded for the current local calendar day across all workspaces
- **AND** achievements recorded on prior days are excluded from those counts

#### Scenario: In-flight count reflects the developer's active changes

- **WHEN** the Dashboard renders
- **THEN** the in-flight count equals the current number of active (non-archived) changes the developer created, counting a change spanning multiple worktrees once
- **AND** when every change the developer created is archived, the in-flight count is `0` regardless of how many changes were created earlier in the day

#### Scenario: Counts lead with shipped then in flight

- **WHEN** the Today's Progress band renders
- **THEN** the four counts appear in the fixed left-to-right order: changes archived (shipped), changes in flight, commits landed, tasks completed

#### Scenario: Comparison to recent daily average

- **WHEN** a today-flow count (shipped, commits, or tasks completed) renders
- **THEN** it shows a comparison indicator relative to the user's trailing recent-day average for that achievement type

#### Scenario: In-flight count has no average comparison

- **WHEN** the in-flight count renders
- **THEN** it shows no average-comparison indicator

#### Scenario: Reduced motion disables the count-up

- **WHEN** the viewer's `prefers-reduced-motion` setting is active
- **THEN** the counts render immediately at their final values without animation

#### Scenario: A day with no today-flow activity

- **WHEN** no changes were archived, no commits landed, and no tasks were completed for the current day
- **THEN** the Today's Progress band renders an encouraging zero state
- **AND** the encouraging zero state is independent of the in-flight count, which may be non-zero
- **AND** it does not render a negative or error state

### Requirement: Streak and Contribution Heatmap

The Dashboard SHALL present a current streak — the number of consecutive local calendar days, ending today, on which at least one achievement was recorded — and a contribution heatmap over a bounded multi-week window in which each cell's intensity reflects that day's achievement count and the current day's cell is visually distinguished. A local calendar day with no recorded achievement SHALL break the streak. The heatmap window SHALL be bounded.

#### Scenario: Streak counts consecutive active days

- **WHEN** achievements were recorded on each of the last N consecutive days ending today
- **THEN** the streak reports N

#### Scenario: A gap breaks the streak

- **WHEN** a day within an otherwise-consecutive run recorded no achievement
- **THEN** the streak counts only the consecutive active days ending today, stopping at the gap

#### Scenario: Heatmap intensity reflects per-day activity

- **WHEN** the heatmap renders
- **THEN** each day's cell intensity corresponds to that day's recorded achievement count
- **AND** the current day's cell is visually distinguished from the others

#### Scenario: Selecting a day reveals its breakdown

- **WHEN** the user selects a day's cell in the heatmap
- **THEN** the Dashboard reveals that day's per-kind achievement breakdown in the order changes shipped, changes started (created that day), commits, tasks completed
- **AND** the per-day "started" breakdown reflects changes created on that specific day, which has no equivalent in the band's live in-flight count
- **AND** a day with no recorded activity reveals an explicit empty state rather than nothing

#### Scenario: Heatmap window is bounded

- **WHEN** activity exists older than the heatmap window
- **THEN** the heatmap renders only the bounded window and does not require the full history

### Requirement: Live Celebration Moments

While the Dashboard is the active center-pane surface, the completion of a change (its archival) SHALL trigger a celebratory visual effect, and the completion of a task SHALL trigger a quieter visual acknowledgement. These effects SHALL be suppressed when the viewer's `prefers-reduced-motion` setting is active. A celebration SHALL NOT block interaction and SHALL NOT persist beyond a brief animation.

#### Scenario: Confetti on a ship while the Dashboard is active

- **WHEN** the Dashboard is the active surface
- **AND** a change is archived in a registered workspace
- **THEN** a celebratory effect plays briefly
- **AND** interaction with the Dashboard is not blocked

#### Scenario: Quieter acknowledgement on a task completion

- **WHEN** the Dashboard is the active surface
- **AND** a task is completed in a registered workspace
- **THEN** a quieter visual acknowledgement plays
- **AND** it is distinct from and less prominent than the change-shipped celebration

#### Scenario: Reduced motion suppresses celebration

- **WHEN** the viewer's `prefers-reduced-motion` setting is active
- **AND** a change is archived or a task is completed while the Dashboard is active
- **THEN** no motion-based celebration plays

#### Scenario: No celebration when the Dashboard is not active

- **WHEN** the Dashboard is not the active center-pane surface
- **AND** a change is archived
- **THEN** no celebration effect plays on the Dashboard

### Requirement: Developer Profile Surface

The Dashboard SHALL present a developer **profile** surface identifying the canonical developer by the display name from the identity configuration and by an **avatar**. The avatar SHALL be generated locally as a deterministic identicon derived from the developer's normalised identity key, tinted from the application's existing token palette, and SHALL NOT be fetched over the network or transmit identity data off the machine. The avatar SHALL be rendered plainly, carrying no earned finish, overlay, or rank ornament. The profile surface SHALL present the developer's *Me*-scoped streak as a personal highlight alongside the avatar, retaining the encouraging zero state when the developer has no recorded activity.

#### Scenario: Profile shows the developer's name and a local avatar

- **WHEN** the profile surface renders with an identity configured
- **THEN** it shows the canonical display name
- **AND** it shows a locally-generated identicon avatar derived from the developer's identity, with no network request

#### Scenario: Profile reflects the developer's own activity

- **WHEN** the profile surface renders
- **THEN** the streak it shows is computed over the *Me*-scoped achievements

#### Scenario: The avatar carries no ornament

- **WHEN** the profile surface renders
- **THEN** the avatar shows the plain identicon
- **AND** no finish, overlay, or rank ornament is applied to it

#### Scenario: Empty profile is encouraging

- **WHEN** the developer has no recorded *Me*-scoped activity
- **THEN** the profile renders an encouraging zero state rather than an error or a discouraging empty board

### Requirement: Unconditional Progress Layer

The Dashboard's **progress layer** SHALL always be present. It comprises the activity-log-derived views (the Today's Progress hero, the streak, and the contribution heatmap), the per-author leaderboard, the commit garden, and the live celebrations. No setting SHALL gate any part of it: the application SHALL NOT persist a progress-layer preference, SHALL NOT expose a control to disable it in any frontend, and SHALL NOT expose a command to read or write such a preference. The layer SHALL be computed and presented on every Dashboard render, subject only to each surface's own conditions — the leaderboard's more-than-one-author rule, the commit garden's dormant and degraded states, and the viewer's `prefers-reduced-motion` setting, which remains the only suppressor of motion.

#### Scenario: The progress layer renders without opt-in

- **WHEN** the Dashboard renders in a fresh installation with no settings ever changed
- **THEN** the Today's Progress hero, the streak, and the contribution heatmap are shown
- **AND** the commit garden is shown
- **AND** live celebrations are armed

#### Scenario: No control disables the layer

- **WHEN** the Settings surface renders in any frontend
- **THEN** no control to enable or disable the progress layer is offered

#### Scenario: No persisted preference and no command

- **WHEN** the application settings are written
- **THEN** they contain no progress-layer or gamification preference
- **AND** no command to read or write such a preference is exposed on the command surface

#### Scenario: A legacy preference is ignored

- **WHEN** an existing settings file carries a gamification preference written by an earlier version
- **THEN** it is ignored and the progress layer renders regardless of its value
- **AND** the key is not preserved on the next write

#### Scenario: Reduced motion still governs motion

- **WHEN** the viewer's `prefers-reduced-motion` setting is active
- **THEN** the count-up animations and the celebration effects are suppressed
- **AND** the non-motion content of the progress layer still renders

### Requirement: Personal Progress Frame

The activity-log-derived achievement views — the *Today's Progress Hero*'s today-flow counts (changes shipped, commits landed, tasks completed) and the *Streak and Contribution Heatmap* — SHALL count only activity that resolves to the canonical developer, per the `developer-identity` capability's query-time resolution, with author-less legacy events counted as the developer's. This personal (*Me*) resolution is unconditional: the Dashboard SHALL NOT present a control to widen these views to other authors, and SHALL NOT present a control to restrict them to any narrower window than the available history. Cross-author comparison is the concern of the per-author **Leaderboard**, which is not the personal frame. The *Today's Progress Hero*'s in-flight active-change count is likewise the developer's, as specified by that requirement. These views SHALL be computed from the in-memory activity log and the shared git mining; resolving them SHALL NOT trigger a separate git-history re-mine.

#### Scenario: Progress views count only the developer's activity

- **WHEN** the activity log holds achievements by the developer and by other authors
- **THEN** the today-flow, streak, and heatmap views count only the achievements resolving to the developer
- **AND** the Dashboard offers no control to widen them to all authors

#### Scenario: No control to narrow the progress views

- **WHEN** the Dashboard renders its personal frame
- **THEN** the today-flow, streak, and heatmap views cover all available history
- **AND** the Dashboard offers no lens control to restrict them to a narrower window

#### Scenario: Claiming an alias folds activity into the developer's counts

- **WHEN** activity recorded under an identity not yet claimed is excluded from the developer's counts
- **AND** that identity is added as an alias of the developer
- **THEN** the progress views subsequently count that activity, without the activity log being rewritten

### Requirement: Per-Author Leaderboard

The Dashboard SHALL present a per-author **leaderboard** ranking authors by their shipped changes, completed tasks, and commits over the Dashboard's bounded window, derived from the authored achievements and commit authorship. The leaderboard SHALL resolve each observed author through the named-people roster: identities folded onto one person SHALL be **combined into a single row**, summing their shipped changes, completed tasks, and commits, and labelled with that person's custom display name; an observed author not on the roster SHALL keep its raw git label. This roster resolution SHALL be presentational and computed at query time — it SHALL NOT modify any stored event. The leaderboard SHALL render only for history that, **after roster resolution**, holds **more than one distinct author**; for a repository (or an aggregate) whose recorded history resolves to a single author, the leaderboard SHALL be omitted rather than shown as a list of one. The local developer's row SHALL include the developer's live activity in addition to their commit-authored history. The leaderboard SHALL be read-only and computed locally; selecting it SHALL NOT mutate any workspace or git state.

#### Scenario: Leaderboard appears for a multi-author repository

- **WHEN** a registered repository's recorded history holds more than one distinct author
- **THEN** the Dashboard shows a leaderboard ranking those authors by shipped changes, completed tasks, and commits over the window

#### Scenario: Leaderboard is omitted for a solo repository

- **WHEN** all recorded history resolves to a single author
- **THEN** no leaderboard is shown

#### Scenario: The developer's row includes live activity

- **WHEN** the leaderboard renders and the developer has recorded live achievements
- **THEN** the developer's row reflects both their commit-authored history and their live activity

#### Scenario: Folded identities form one summed, named row

- **WHEN** two of an author's git identities are folded onto a single named person on the roster
- **THEN** the leaderboard shows one row for that person, labelled with their custom display name
- **AND** that row sums the shipped changes, completed tasks, and commits of both identities rather than splitting them across two rows

#### Scenario: A custom name labels an author's row

- **WHEN** an observed author is given a custom display name on the roster
- **THEN** the leaderboard labels that author's row with the custom name rather than the raw git name or email

#### Scenario: Merging the only other author omits the leaderboard

- **WHEN** the sole author other than the developer is folded onto the developer
- **THEN** the history resolves to a single author and no leaderboard is shown

#### Scenario: Roster resolution does not rewrite the log

- **WHEN** authors are named or merged on the roster
- **THEN** no stored activity-log event is modified
- **AND** the developer's own personal-frame counts are unchanged

#### Scenario: Leaderboard does not mutate state

- **WHEN** the user interacts with the leaderboard
- **THEN** no spec file, workspace, or git state is modified

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
