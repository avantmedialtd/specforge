# dashboard Specification

## Purpose

Defines the Dashboard: the global, read-only overview rendered as the default home surface of the center pane. It aggregates state across every registered workspace — summary metrics, a per-repository breakdown, a git-mined commits-per-day activity chart, change-lifecycle throughput and time-to-archive, and a recent-activity feed — refreshing on the existing cache and graph events and degrading gracefully when git is unavailable.
## Requirements
### Requirement: Dashboard Home Surface

The application SHALL provide a Dashboard: a global, read-only overview rendered in the center (detail) pane. The Dashboard SHALL be the center pane's default render target — it SHALL be shown at startup and whenever no artifact and no commit is selected, in place of any "nothing selected" placeholder.

The tree pane SHALL render a pinned "Dashboard" entry at the top of the pane (mirroring the pinned Settings entry at the bottom). Selecting the Dashboard entry SHALL set the center pane to the Dashboard. Selecting a renderable artifact in the tree, or a commit in the rail, SHALL replace the Dashboard with that target; selecting the Dashboard entry again SHALL return the center pane to the Dashboard. The Dashboard entry SHALL convey an active treatment while the Dashboard is the current center-pane target.

#### Scenario: Dashboard shown at startup

- **WHEN** the user opens the main window and no artifact or commit has been selected
- **THEN** the center pane renders the Dashboard
- **AND** no "nothing selected" placeholder is shown

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

The Dashboard SHALL present summary metrics aggregated across every registered workspace:

- the total number of active (non-archived) changes,
- a task rollup — the sum of completed tasks and the sum of total tasks across all active changes, expressed as both a `completed / total` count and a percentage,
- the number of active changes that touch at least one capability spec,
- the number of registered repositories, worktrees, and non-git (flat) workspaces.

The percentage SHALL be defined as `0` when the total task count is zero, and the rollup SHALL never divide by zero.

#### Scenario: Summary counts reflect all workspaces

- **WHEN** the Dashboard renders with multiple registered workspaces
- **THEN** the active-change count equals the total number of non-archived changes across all of them
- **AND** the task rollup equals the summed completed and total task counts across those changes

#### Scenario: Task rollup with no tasks

- **WHEN** every active change across all workspaces has zero parsed tasks
- **THEN** the task rollup shows `0 / 0` and a percentage of `0`
- **AND** no division-by-zero error occurs

#### Scenario: Empty registry

- **WHEN** no workspaces are registered
- **THEN** the Dashboard renders without error
- **AND** the summary metrics show zero counts

### Requirement: Per-Repository Breakdown

The Dashboard SHALL present a breakdown with one entry per top-level registered item — a repository group or a non-git (flat) workspace — showing that entry's count of active changes and its count of archived changes. Each entry SHALL be labelled with the same display name the tree pane uses for that top-level row.

#### Scenario: One row per top-level entry

- **WHEN** the Dashboard renders with two repositories and one flat workspace registered
- **THEN** the breakdown shows three entries
- **AND** each entry shows its active-change count and its archived-change count

#### Scenario: Breakdown labels match the tree

- **WHEN** a repository group has a configured display name
- **THEN** its breakdown entry is labelled with that same display name

### Requirement: Git-Mined Activity Chart

The Dashboard SHALL present an activity chart showing the number of commits per calendar day over a recent bounded window, aggregated across every git-backed registered repository. Commit dates SHALL be bucketed by calendar day in the viewer's local time zone, consistent with the commit-graph rail's day grouping. The window SHALL be bounded (the chart SHALL NOT require reading a repository's entire history).

Commit activity SHALL be derived from git history. A repository contributes commits to the chart only when it is git-backed and `git` is available; non-git (flat) workspaces contribute nothing to the chart.

#### Scenario: Chart aggregates commits across repositories

- **WHEN** two git-backed repositories each received commits within the window
- **THEN** each day's bar in the chart reflects the combined commit count from both repositories for that day

#### Scenario: Window is bounded

- **WHEN** a repository contains commits older than the chart's window
- **THEN** commits outside the window are not included in the chart
- **AND** the chart does not read the repository's entire history to render

#### Scenario: Non-git workspaces do not contribute

- **WHEN** a registered workspace is not inside a git repository
- **THEN** it contributes no data to the activity chart
- **AND** the chart renders using only the git-backed repositories' commits

### Requirement: Change Lifecycle Metrics

The Dashboard SHALL present change-lifecycle metrics derived from git history: the throughput (the number of changes archived within the recent window) and the average time-to-archive (the mean elapsed time between a change's creation and its archival). A change's creation date SHALL be the date of the earliest commit that added a file under its `openspec/changes/<id>/` directory; its archive date SHALL be the date of the earliest commit that added a file under `openspec/changes/archive/<id>/`. Only changes for which both dates are recoverable from git SHALL contribute to the average time-to-archive.

#### Scenario: Throughput counts recent archives

- **WHEN** three changes were archived within the window and others were archived earlier
- **THEN** the throughput metric reports the three changes archived within the window

#### Scenario: Average time-to-archive uses recoverable lifecycles

- **WHEN** a set of changes has both a creation commit and an archive commit recoverable from git
- **THEN** the average time-to-archive is the mean of each such change's archive-date-minus-creation-date
- **AND** changes whose creation or archive date cannot be recovered from git are excluded from the average

#### Scenario: No recoverable lifecycles

- **WHEN** no change has both a recoverable creation and archive date
- **THEN** the average time-to-archive renders as unavailable rather than as an error or a zero average

### Requirement: Recent Activity Feed

The Dashboard SHALL present a recent-activity feed: a list of recently active changes across all workspaces, ordered most-recent first by modification time. Each feed entry SHALL identify its change and its owning workspace or repository. Selecting a feed entry SHALL navigate the application to that change (for example, by rendering one of its artifacts in the center pane), consistent with the existing tree-selection navigation.

#### Scenario: Feed is ordered by recency

- **WHEN** the recent-activity feed renders
- **THEN** entries are ordered most-recently-modified first

#### Scenario: Selecting a feed entry navigates to the change

- **WHEN** the user selects an entry in the recent-activity feed
- **THEN** the application navigates to that change
- **AND** the navigation uses the same selection contract as selecting the change in the tree

### Requirement: Reactive Dashboard Updates

While the Dashboard is the active center-pane surface, it SHALL reflect on-disk changes within the watcher's debounce window without user action. After the watcher finishes processing a debounced batch — a change added, a change archived, content edited within a tracked change, or a repository's refs changing — the Dashboard SHALL refresh its metrics to observe the post-batch state.

#### Scenario: Dashboard updates when a change is added

- **WHEN** the Dashboard is the active surface
- **AND** a new change directory is created on disk in a registered workspace
- **THEN** the Dashboard's active-change count and recent-activity feed reflect the new change within the debounce window

#### Scenario: Dashboard updates when a change is archived

- **WHEN** the Dashboard is the active surface
- **AND** a change is moved to `openspec/changes/archive/` on disk
- **THEN** the Dashboard's active/archived counts and lifecycle metrics reflect the archival within the debounce window

#### Scenario: Dashboard updates on commit activity

- **WHEN** the Dashboard is the active surface
- **AND** a new commit is created in a registered git-backed repository
- **THEN** the activity chart reflects the new commit within the debounce window

### Requirement: Graceful Degradation Without Git

When the `git` binary is unavailable, or a registered workspace is not inside a git repository, the Dashboard SHALL still render its non-git sections (summary metrics, per-repository breakdown, recent-activity feed) and SHALL render the git-derived sections (activity chart, lifecycle metrics) using only the data recoverable from the available git-backed repositories. The Dashboard SHALL NOT error when git is absent.

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** the Dashboard renders its summary metrics, per-repository breakdown, and recent feed
- **AND** the activity chart and lifecycle metrics render an empty or unavailable state rather than erroring

#### Scenario: Mixed git and non-git workspaces

- **WHEN** some registered workspaces are git-backed and others are flat
- **THEN** the summary metrics and breakdown include every workspace
- **AND** the activity chart and lifecycle metrics include only the git-backed repositories

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

The Dashboard SHALL present a "Today's Progress" band as its topmost content, showing four headline counts aggregated across all registered workspaces. Three of the counts reflect achievements recorded for the current local calendar day — changes archived (shipped), commits landed, and tasks completed. The remaining count reflects the *current* number of active (non-archived) changes the developer has in flight — the changes the developer created, consistent with the personal resolution of the gamified frame — which is a live state count rather than a today count. The counts SHALL be presented as a fixed left-to-right sequence: changes archived (shipped), changes in flight, commits landed, then tasks completed — so the two change-level counts lead as a pair and the within-change increments follow. Each count SHALL render with an animated count-up on first render. Each of the three today-flow counts (shipped, commits, tasks completed) SHALL be accompanied by a comparison to the user's recent daily average for that achievement type; the in-flight count SHALL NOT show an average comparison, as a live level has no trailing daily average. When the viewer's `prefers-reduced-motion` setting is active, counts SHALL render at their final value without animation. The day boundary for the today-flow counts SHALL be the viewer's local calendar day, consistent with the commit-graph rail's day grouping.

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

The Dashboard SHALL present a developer **profile** surface identifying the canonical developer by the display name from the identity configuration and by an **avatar**. The avatar SHALL be generated locally as a deterministic identicon derived from the developer's normalised identity key, tinted from the application's existing token palette, and SHALL NOT be fetched over the network or transmit identity data off the machine. When the gamified layer is enabled and a treatment is equipped, the avatar SHALL carry that equipped treatment finish, per the *Equipped Badge Treatments* requirement. The profile surface SHALL present the developer's *Me*-scoped streak as a personal highlight alongside the avatar, retaining the encouraging zero state when the developer has no recorded activity.

#### Scenario: Profile shows the developer's name and a local avatar

- **WHEN** the profile surface renders with an identity configured
- **THEN** it shows the canonical display name
- **AND** it shows a locally-generated identicon avatar derived from the developer's identity, with no network request

#### Scenario: Profile reflects the developer's own activity

- **WHEN** the profile surface renders
- **THEN** the streak it shows is computed over the *Me*-scoped achievements

#### Scenario: Empty profile is encouraging

- **WHEN** the developer has no recorded *Me*-scoped activity
- **THEN** the profile renders an encouraging zero state rather than an error or a discouraging empty board

### Requirement: Per-Author Leaderboard for Shared Repositories

The Dashboard SHALL present a per-author **leaderboard** ranking authors by their shipped changes, completed tasks, and commits over the Dashboard's bounded window, derived from the authored achievements and commit authorship. The leaderboard SHALL render only for history that holds **more than one distinct author**; for a repository (or an aggregate) whose recorded history has a single author, the leaderboard SHALL be omitted rather than shown as a list of one. The local developer's row SHALL include the developer's live activity in addition to their commit-authored history. The leaderboard SHALL be read-only and computed locally; selecting it SHALL NOT mutate any workspace or git state.

#### Scenario: Leaderboard appears for a multi-author repository

- **WHEN** a registered repository's recorded history holds more than one distinct author
- **THEN** the Dashboard shows a leaderboard ranking those authors by shipped changes, completed tasks, and commits over the window

#### Scenario: Leaderboard is omitted for a solo repository

- **WHEN** all recorded history resolves to a single author
- **THEN** no leaderboard is shown

#### Scenario: The developer's row includes live activity

- **WHEN** the leaderboard renders and the developer has recorded live achievements
- **THEN** the developer's row reflects both their commit-authored history and their live activity

#### Scenario: Leaderboard does not mutate state

- **WHEN** the user interacts with the leaderboard
- **THEN** no spec file, workspace, or git state is modified

### Requirement: Season Home on the Profile Band

The Dashboard's profile band SHALL present a **season home** for the active season: its generated name, a countdown to the season's end, the current band and tier with the gap to the next tier, the battle-pass track with the next unlock previewed, the active objectives with their progress, and the developer's equipped treatment. The season home SHALL be Me-scoped and SHALL retain an encouraging zero state when the developer has no season activity yet.

#### Scenario: Season home shows the active season

- **WHEN** the profile band renders
- **THEN** it shows the season name, the end countdown, the current band and tier with the gap to the next tier, the battle-pass track with the next unlock previewed, the active objectives with their progress, and the equipped treatment

#### Scenario: Encouraging zero state

- **WHEN** the developer has no recorded activity in the active season
- **THEN** the season home renders an encouraging zero state rather than an error or a discouraging empty board

### Requirement: Permanent Career Tier Readout

The Dashboard SHALL present the developer's **career tier** — the permanent tier derived from lifetime cumulative totals — rendered distinctly from the seasonal band, so the resetting seasonal standing and the permanent career standing are not confused.

#### Scenario: Career tier shown distinctly

- **WHEN** the profile band renders
- **THEN** the permanent career tier is shown and is visually distinct from the resetting seasonal band

#### Scenario: Career tier persists across a reset

- **WHEN** a new season resets the seasonal band
- **THEN** the career tier readout is unchanged

### Requirement: Equipped Badge Treatments

The Dashboard SHALL render the developer's **equipped treatment** as a finish over their **profile avatar** (the identicon). Browsing the locker of unlocked finishes and choosing which one is equipped SHALL be a **Settings** surface ("Badge finishes"), not the Dashboard — the Dashboard reflects the equipped finish but does not host the picker. Rendering an equipped treatment SHALL make no network request, and an animated finish SHALL be suppressed when the viewer's `prefers-reduced-motion` setting is active.

#### Scenario: Equipped treatment renders on the avatar

- **WHEN** a treatment is equipped
- **THEN** the developer's profile avatar renders with that finish

#### Scenario: Equipping happens in Settings

- **WHEN** the developer selects a different unlocked treatment from the Settings badge-finishes locker
- **THEN** it becomes the equipped finish
- **AND** the Dashboard renders the avatar with it

#### Scenario: Reduced motion suppresses an animated finish

- **WHEN** the viewer's `prefers-reduced-motion` setting is active
- **AND** the equipped treatment is animated
- **THEN** its motion is suppressed

### Requirement: Seasonal Leaderboard Variant

For shared repositories whose history holds more than one author, the Dashboard SHALL offer a **season-scoped** variant of the per-author leaderboard, ranking authors over the active season's window, alongside the existing all-time leaderboard. The seasonal leaderboard SHALL be omitted for single-author history, SHALL be read-only and computed locally, and SHALL NOT mutate any workspace or git state.

#### Scenario: Seasonal leaderboard for multi-author history

- **WHEN** a repository's recorded history holds more than one distinct author
- **THEN** a season-windowed leaderboard ranks those authors over the active season

#### Scenario: Omitted for solo history

- **WHEN** all recorded history resolves to a single author
- **THEN** the seasonal leaderboard is omitted

#### Scenario: Read-only

- **WHEN** the user interacts with the seasonal leaderboard
- **THEN** no spec file, workspace, or git state is modified

### Requirement: Live Tier-Up Acknowledgement

While the Dashboard is the active center-pane surface, crossing a battle-pass tier from **live** (non-backfilled) season activity SHALL trigger a brief tier-up acknowledgement consistent with the existing celebration treatment — suppressed when the viewer's `prefers-reduced-motion` setting is active, non-blocking, and not persisting beyond a brief animation. A tier crossed by backfilled history SHALL NOT trigger a live acknowledgement.

#### Scenario: Tier-up on live progress

- **WHEN** the Dashboard is the active surface
- **AND** live season activity crosses a battle-pass tier
- **THEN** a brief tier-up acknowledgement plays
- **AND** interaction with the Dashboard is not blocked

#### Scenario: Reduced motion suppresses the acknowledgement

- **WHEN** the viewer's `prefers-reduced-motion` setting is active
- **AND** a battle-pass tier is crossed
- **THEN** no motion-based acknowledgement plays

#### Scenario: Backfilled tiers are silent

- **WHEN** a battle-pass tier is crossed by backfilled history
- **THEN** no live acknowledgement plays

### Requirement: Gamification Opt-In

The gamified progress layer SHALL be gated behind a setting that is **disabled by default**. The gated layer comprises the gamified, activity-log-derived views (today's progress, streak, contribution heatmap), the live celebrations, the per-author leaderboard, and every season surface (the season home, the equipped-treatment finish on the avatar, the seasonal leaderboard, the live tier-up, and the permanent career-tier readout). When the setting is disabled, the Dashboard SHALL render only its analytics — the cross-workspace summary metrics, per-repository breakdown, git-mined activity chart, change-lifecycle metrics, and recent-activity feed — and SHALL NOT compute or present any gated section; the Settings *Badge finishes* surface SHALL likewise be hidden. Enabling the setting SHALL restore the gamified layer. The setting SHALL persist in the application's data directory.

#### Scenario: Gamification is off by default

- **WHEN** the gamification setting has never been enabled
- **THEN** the gamified layer is disabled
- **AND** the Dashboard renders only its analytics sections

#### Scenario: Enabling restores the gamified layer

- **WHEN** the gamification setting is enabled
- **THEN** the Dashboard presents the gamified layer — today's progress, streak, heatmap, the season surfaces, the leaderboard, and celebrations

#### Scenario: Disabled hides the Settings locker

- **WHEN** gamification is disabled
- **THEN** the Settings badge-finishes locker is not shown

#### Scenario: Disabled skips gamified computation

- **WHEN** gamification is disabled
- **THEN** the gamified sections are not computed for the Dashboard payload

### Requirement: Personal Gamified Frame

The gamified, activity-log-derived achievement views — the *Today's Progress Hero*'s today-flow counts (changes shipped, commits landed, tasks completed) and the *Streak and Contribution Heatmap* — SHALL count only activity that resolves to the canonical developer, per the `developer-identity` capability's query-time resolution, with author-less legacy events counted as the developer's. This personal (*Me*) resolution is unconditional: the Dashboard SHALL NOT present a control to widen these views to other authors, and SHALL NOT present a control to restrict them to a single season's window. Cross-author comparison is the concern of the per-author **Leaderboard** (and its seasonal variant); the active season's standing is the concern of the **season home** and the **seasonal leaderboard** — none of which is the personal frame. The *Today's Progress Hero*'s in-flight active-change count is likewise the developer's, as specified by that requirement. These views SHALL be computed from the in-memory activity log and the shared git mining; resolving them SHALL NOT trigger a separate git-history re-mine.

#### Scenario: Gamified views count only the developer's activity

- **WHEN** the activity log holds achievements by the developer and by other authors
- **THEN** the today-flow, streak, and heatmap views count only the achievements resolving to the developer
- **AND** the Dashboard offers no control to widen them to all authors

#### Scenario: No control to narrow the gamified views to a season

- **WHEN** the Dashboard renders its gamified frame
- **THEN** the today-flow, streak, and heatmap views cover all available history
- **AND** the Dashboard offers no lens control to restrict them to the active season's window

#### Scenario: Claiming an alias folds activity into the developer's counts

- **WHEN** activity recorded under an identity not yet claimed is excluded from the developer's counts
- **AND** that identity is added as an alias of the developer
- **THEN** the gamified views subsequently count that activity, without the activity log being rewritten

