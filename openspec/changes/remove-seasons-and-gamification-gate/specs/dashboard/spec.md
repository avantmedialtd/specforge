## ADDED Requirements

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

## MODIFIED Requirements

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

### Requirement: Per-Author Leaderboard for Shared Repositories

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

### Requirement: Dashboard Unaffected by Workspace Disable

Disabling a top-level row (see the *Workspace Disable State* requirement in the
`workspace-registry` capability) SHALL have no effect on any Dashboard surface.
A disabled workspace SHALL continue to contribute to the cross-workspace summary
metrics, the per-repository breakdown, the git-mined activity chart, the change
lifecycle metrics, today's ships feed, the today's-progress hero, and the streak
and contribution heatmap.

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
and never the git-derived working-tree fields, a disabled row's omitted git state
SHALL NOT degrade any Dashboard figure.

#### Scenario: Summary metrics include disabled workspaces

- **WHEN** two workspaces are registered, one enabled with five active changes and one disabled with four
- **THEN** the Dashboard's active-change summary reports nine
- **AND** the tree pane shows only the enabled workspace's five

#### Scenario: Per-repository breakdown keeps a row for a disabled workspace

- **WHEN** a registered repository is disabled
- **THEN** the Dashboard's breakdown still shows an entry for it
- **AND** that entry shows its active-change and archived-change counts
- **AND** it is labelled with the same display name it had before being disabled

#### Scenario: Activity chart and lifecycle metrics include disabled repositories

- **WHEN** a disabled repository received commits within the chart's window
- **THEN** those commits are reflected in the activity chart's daily buckets
- **AND** the repository's changes contribute to the lifecycle throughput metrics

#### Scenario: Ships from a disabled workspace still appear

- **WHEN** a change in a disabled workspace is archived today
- **THEN** it appears in today's ships feed
- **AND** the entry is marked as belonging to a disabled workspace
- **AND** selecting it leads to the settings view where the workspace can be re-enabled, rather than doing nothing (see the *Ship Selection Opens the Archive Browser* requirement)

#### Scenario: The disabled-workspace note counts rows, not registered folders

- **WHEN** one repository is registered at two worktrees and is disabled
- **THEN** the Dashboard's note reports one disabled workspace
- **AND** the tree pane has dropped exactly one top-level row

#### Scenario: Streak and heatmap are unaffected

- **WHEN** a workspace is disabled for a period during which the user completes tasks and archives changes in it
- **THEN** those days count toward the streak and the contribution heatmap
- **AND** no streak day is lost as a result of the workspace having been disabled

#### Scenario: Dashboard renders when every workspace is disabled

- **WHEN** every registered workspace is disabled
- **THEN** the Dashboard renders without error
- **AND** its summary metrics, breakdown, and activity chart still reflect all registered workspaces
- **AND** the tray badge is hidden and the tree pane is empty

## REMOVED Requirements

### Requirement: Gamification Opt-In

**Reason**: The opt-in gate is removed; the progress layer is unconditional. Replaced by the *Unconditional Progress Layer* requirement.

### Requirement: Personal Gamified Frame

**Reason**: Renamed to *Personal Progress Frame* (added above), which carries the same Me-scoping invariant without the season-lens clause.

### Requirement: Season Home on the Profile Band

**Reason**: The season / battle-pass system is removed in full.

### Requirement: Permanent Career Tier Readout

**Reason**: The career tier is an invented ladder over lifetime totals and the last consumer of the `seasons` module; it is removed with the rest of the scoring system.

### Requirement: Equipped Badge Treatments

**Reason**: Badge finishes are removed; the profile avatar reverts to the plain identicon.

### Requirement: Seasonal Leaderboard Variant

**Reason**: The season window no longer exists; the all-time per-author leaderboard is retained unchanged.

### Requirement: Live Tier-Up Acknowledgement

**Reason**: There are no battle-pass tiers to cross. The ship and task celebrations are retained by *Live Celebration Moments*.
