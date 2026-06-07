## ADDED Requirements

### Requirement: Personal Gamified Frame

The gamified, activity-log-derived achievement views — the *Today's Progress Hero*'s today-flow counts (changes shipped, commits landed, tasks completed), the *Streak and Contribution Heatmap*, and the *Milestones and Badges* — SHALL count only activity that resolves to the canonical developer, per the `developer-identity` capability's query-time resolution, with author-less legacy events counted as the developer's. This personal (*Me*) resolution is unconditional: the Dashboard SHALL NOT present a control to widen these views to other authors, and SHALL NOT present a control to restrict them to a single season's window. Cross-author comparison is the concern of the per-author **Leaderboard** (and its seasonal variant); the active season's standing is the concern of the **season home** and the **seasonal leaderboard** — none of which is the personal frame. The *Today's Progress Hero*'s in-flight active-change count is likewise the developer's, as specified by that requirement. These views SHALL be computed from the in-memory activity log and the shared git mining; resolving them SHALL NOT trigger a separate git-history re-mine.

#### Scenario: Gamified views count only the developer's activity

- **WHEN** the activity log holds achievements by the developer and by other authors
- **THEN** the today-flow, streak, heatmap, and milestone views count only the achievements resolving to the developer
- **AND** the Dashboard offers no control to widen them to all authors

#### Scenario: No control to narrow the gamified views to a season

- **WHEN** the Dashboard renders its gamified frame
- **THEN** the today-flow, streak, heatmap, and milestone views cover all available history
- **AND** the Dashboard offers no lens control to restrict them to the active season's window

#### Scenario: Claiming an alias folds activity into the developer's counts

- **WHEN** activity recorded under an identity not yet claimed is excluded from the developer's counts
- **AND** that identity is added as an alias of the developer
- **THEN** the gamified views subsequently count that activity, without the activity log being rewritten

## MODIFIED Requirements

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

### Requirement: Gamification Opt-In

The gamified progress layer SHALL be gated behind a setting that is **disabled by default**. The gated layer comprises the gamified, activity-log-derived views (today's progress, streak, contribution heatmap, milestones), the live celebrations, the per-author leaderboard, and every season surface (the season home, the equipped-treatment finish on badges, the seasonal leaderboard, the live tier-up, and the permanent career-tier readout). When the setting is disabled, the Dashboard SHALL render only its analytics — the cross-workspace summary metrics, per-repository breakdown, git-mined activity chart, change-lifecycle metrics, and recent-activity feed — and SHALL NOT compute or present any gated section; the Settings *Badge finishes* surface SHALL likewise be hidden. Enabling the setting SHALL restore the gamified layer. The setting SHALL persist in the application's data directory.

#### Scenario: Gamification is off by default

- **WHEN** the gamification setting has never been enabled
- **THEN** the gamified layer is disabled
- **AND** the Dashboard renders only its analytics sections

#### Scenario: Enabling restores the gamified layer

- **WHEN** the gamification setting is enabled
- **THEN** the Dashboard presents the gamified layer — today's progress, streak, heatmap, milestones, the season surfaces, the leaderboard, and celebrations

#### Scenario: Disabled hides the Settings locker

- **WHEN** gamification is disabled
- **THEN** the Settings badge-finishes locker is not shown

#### Scenario: Disabled skips gamified computation

- **WHEN** gamification is disabled
- **THEN** the gamified sections are not computed for the Dashboard payload

## REMOVED Requirements

### Requirement: Activity Scope Selection (Me / Everyone)

**Reason:** The *Me / Everyone* segmented control is removed. Applying an *Everyone* audience to an intrinsically first-person gamified frame (your streak, your milestones, your flame) produces incoherent aggregates — an "everyone's streak" is the union of unrelated authors' active days, owned by no one. The legitimate team-comparison view is already owned, and better expressed, by the per-author **Leaderboard** (ranked per author, not melted into one total). The surviving invariant — that the gamified views are unconditionally resolved to the canonical developer — is carried by the added **Personal Gamified Frame** requirement, which also rehomes the alias-claiming scenario. The default scope was already *Me*, so this removes a lever rather than changing the default view.

### Requirement: Season Lens (This Season / All Time)

**Reason:** The *This Season / All Time* segmented control is removed. The today, streak, heatmap, and milestone views are inherently *cumulative personal* facts, so a season-window truncation of them is the same category error as an "everyone's streak"; the active season's standing already has dedicated homes in the season home and the seasonal leaderboard, which carry their own season-window logic independent of this control. The default lens was already *All Time*, so the gamified tiles keep showing all-time history with no behavioural change to the default view.
