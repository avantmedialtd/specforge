## ADDED Requirements

### Requirement: Change Lifecycle Mining

The Dashboard SHALL derive each registered repository's change lifecycles from git history, so that today's ships feed can present each entry's archival instant. A change's creation date SHALL be the date of the earliest commit that added a file under its `openspec/changes/<id>/` directory; its archive date SHALL be the date of the earliest commit that added a file under `openspec/changes/archive/<id>/`.

Lifecycle data is derived from append-only history — once a change's creation and archival commits exist, their instants do not change — so the application SHALL derive a repository's lifecycle data **at most once per change to that repository's history**, rather than once per Dashboard fetch. A Dashboard fetch whose repositories' histories have not moved since the previous fetch SHALL issue no lifecycle-mining git invocation.

When a repository's history moves, the application SHALL re-derive that repository's lifecycle data and SHALL NOT re-derive any other repository's. The Dashboard SHALL reflect the moved history within the watcher's debounce window, so the freshness the *Reactive Dashboard Updates* requirement guarantees is unchanged.

Concurrent derivations for the same repository SHALL be collapsed into a single mining invocation, so that overlapping fetches cannot each issue their own.

The derived lifecycles SHALL be identical to those a per-fetch derivation would produce for the same history.

The Dashboard SHALL NOT present aggregate lifecycle statistics — neither a count of changes archived within a window, nor an average time-to-archive, nor any window naming such a figure.

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

- **WHEN** two Dashboard fetches overlap for a repository whose history has moved
- **THEN** that repository's lifecycle data is mined once
- **AND** both fetches reflect the same derived lifecycles

#### Scenario: Mined lifecycles date the ships feed

- **WHEN** a change's archival commit is recoverable from git
- **THEN** its entry in today's ships feed presents that archival instant

#### Scenario: No aggregate lifecycle statistics are presented

- **WHEN** the Dashboard renders
- **THEN** no count of changes archived within a window is presented
- **AND** no average time-to-archive is presented
- **AND** no throughput window length is presented

## MODIFIED Requirements

### Requirement: Cross-Workspace Summary Metrics

The Dashboard SHALL present, aggregated across every registered workspace, the total number of active (non-archived) changes — rendered as a compact summary line alongside the total archived count, not as a metric card. The Dashboard SHALL NOT present standalone Overview cards for the task rollup, for the count of active changes that touch a capability spec, or for the registered repository/worktree counts.

The Dashboard's underlying cross-workspace data SHALL retain every top-level registered item — a repository group or a non-git (flat) workspace — with that item's count of active changes and its count of archived changes, so that the totals derived from it remain complete regardless of what any surface presents. The Dashboard SHALL NOT present that per-item data as a breakdown of its own: no per-repository list, ranking, cap, remainder line or proportional bar SHALL be rendered. Where a per-item active-change count is presented, it SHALL be presented as an annotation on a surface that has its own reason to name that item.

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

#### Scenario: No per-repository breakdown is rendered

- **WHEN** the Dashboard renders with several registered workspaces
- **THEN** no per-repository list, ranking, cap, remainder line or proportional bar is presented

#### Scenario: Summary totals remain complete

- **WHEN** more top-level items are registered than any surface names
- **THEN** the archived total in the summary line still aggregates every registered workspace

### Requirement: Graceful Degradation Without Git

When the `git` binary is unavailable, or a registered workspace is not inside a
git repository, the Dashboard SHALL still render its non-git sections (summary
metrics, today's ships feed) using only the data recoverable from the available
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
- **THEN** the Dashboard renders its summary metrics and today's ships feed
- **AND** the today's ships entries render without their relative archive times
- **AND** the Dashboard does not error

#### Scenario: Mixed git and non-git workspaces

- **WHEN** some registered workspaces are git-backed and others are flat
- **THEN** the summary metrics include every workspace
- **AND** only the git-backed repositories contribute mined lifecycles

#### Scenario: A transient mining failure is retried

- **WHEN** a repository's lifecycle derivation fails
- **AND** the Dashboard is fetched again
- **THEN** the derivation is retried for that repository
- **AND** a subsequent successful derivation dates that repository's ships entries

#### Scenario: A repository with no changes is retained

- **WHEN** a repository's lifecycle derivation succeeds and finds no changes
- **AND** the Dashboard is fetched again with that repository's history unmoved
- **THEN** no further lifecycle-mining invocation is issued for that repository

### Requirement: Dashboard Section Order

The Dashboard SHALL present its sections in a fixed vertical order: the
developer profile and streak, today's progress counts, today's ships feed, the
contribution heatmap, then the commit garden. That order SHALL NOT depend on
whether a section is in its quiet state, so no section changes position as the
day's activity accumulates.

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

#### Scenario: The commit garden is last

- **WHEN** the Dashboard renders
- **THEN** the commit garden is the final section of the Dashboard's content
- **AND** no analytics band is rendered between the heatmap and the garden

#### Scenario: The terminal frontend agrees on the order

- **WHEN** the terminal frontend renders its dashboard screen
- **THEN** its ships-today section appears above its activity section

### Requirement: Dashboard Includes Disabled Workspaces

Disabling a top-level row (see the *Workspace Disable State* requirement in the
`workspace-registry` capability) SHALL have no effect on any Dashboard surface.
A disabled workspace SHALL continue to contribute to the cross-workspace summary
metrics, the mined change lifecycles, today's ships feed, the today's-progress
hero, the streak and contribution heatmap, and the commit garden.

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

#### Scenario: A disabled workspace still contributes

- **WHEN** a registered top-level row is disabled
- **THEN** its active and archived changes still contribute to the Dashboard's summary metrics
- **AND** its commits still appear in the commit garden
- **AND** its archived changes still appear in today's ships feed

#### Scenario: The note counts disabled rows

- **WHEN** a repository registered at several worktrees is disabled
- **THEN** the Dashboard's note counts one disabled top-level row rather than one per registered folder

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
- **AND** the per-day "started" breakdown reflects changes created on that specific day, which has no equivalent in the today's-progress hero's live in-flight count
- **AND** a day with no recorded activity reveals an explicit empty state rather than nothing

#### Scenario: Heatmap window is bounded

- **WHEN** activity exists older than the heatmap window
- **THEN** the heatmap renders only the bounded window and does not require the full history

### Requirement: Personal Progress Frame

The activity-log-derived achievement views — the *Today's Progress Hero*'s today-flow counts (changes shipped, commits landed, tasks completed) and the *Streak and Contribution Heatmap* — SHALL count only activity that resolves to the canonical developer, per the `developer-identity` capability's query-time resolution, with author-less legacy events counted as the developer's. This personal (*Me*) resolution is unconditional: the Dashboard SHALL NOT present a control to widen these views to other authors, and SHALL NOT present a control to restrict them to any narrower window than the available history. Cross-author comparison is outside the Dashboard's concern entirely: the Dashboard SHALL NOT rank, score, or otherwise order authors against one another, so the personal frame is the only frame these views have. This prohibition governs the ordering of *authors*; it does not restrict surfaces that merely present several authors without ranking them, such as the commit garden's per-author node colouring, nor the ordering of repositories required by the `commit-garden` capability's *Deterministic Plot Order* requirement. The *Today's Progress Hero*'s in-flight active-change count is likewise the developer's, as specified by the *Today's Progress Hero* requirement; the commit garden's per-entry active-change count is not, being registry-wide per the *Cross-Workspace Summary Metrics* requirement. These views SHALL be computed from the in-memory activity log and the shared git mining; resolving them SHALL NOT trigger a separate git-history re-mine.

#### Scenario: Achievement views count only the developer

- **WHEN** commits by several authors landed on the current local day
- **THEN** the today-flow commit count counts only the developer's

#### Scenario: No control widens the frame

- **WHEN** the Dashboard renders in any frontend
- **THEN** no control to widen the achievement views to other authors is offered

#### Scenario: Repository ordering is not author ranking

- **WHEN** the commit garden orders its plots
- **THEN** that ordering is permitted, being an ordering of repositories rather than of authors

#### Scenario: The two active counts are distinguished

- **WHEN** the Dashboard presents the hero's in-flight count and a commit-garden plot's active count
- **THEN** the hero's count is the developer's own
- **AND** the plot's count is that entry's registry-wide active-change count

## REMOVED Requirements

### Requirement: Analytics Band Composition

**Reason**: The analytics band is removed. It contained one panel — the per-repository breakdown — under two headings, and its divider rule carried three lifecycle figures. The breakdown was a capped decomposition of the two registry-wide totals the Dashboard's footnote already presents, and a worse answer to "which repository holds my work" than the uncapped tree pane. The lifecycle figures were fourteen-day trend aggregates on a surface whose every other element answers today or right now; no requirement depended on their being displayed and no user action followed from them. Its replacement is nothing: the commit garden, already the Dashboard's other per-entry surface, becomes the only one, and carries the single figure worth keeping as a caption annotation (`commit-garden`: *Plot Caption*).

### Requirement: Per-Repository Breakdown

**Reason**: The breakdown's presentation is removed with the band that contained it — the ranked rows, the fixed maximum of five entries, the remainder line, the proportional bars and the two row shapes all go. Its one surviving clause, that the underlying cross-workspace data retains every top-level item so the summary line's totals stay complete, moves to *Cross-Workspace Summary Metrics*, which is the requirement that consumes it. The three-key ordering is not carried over: it existed so rendered rows would not trade places between refreshes, and there are no rows.

### Requirement: Change Lifecycle Metrics

**Reason**: Replaced by *Change Lifecycle Mining* above. The presented metrics — the windowed archive throughput, the average time-to-archive, and the window that named them — are removed. Everything else the requirement specified is retained under the new name: the git-derived creation and archival dates, the at-most-once-per-history-change derivation, the per-repository invalidation, the collapsing of concurrent derivations, and the equivalence to a per-fetch derivation. The rename is deliberate rather than cosmetic — the mining remains load-bearing for today's ships feed's archival instants, and a requirement named for metrics it no longer specifies would misdirect the next reader into deleting it.
