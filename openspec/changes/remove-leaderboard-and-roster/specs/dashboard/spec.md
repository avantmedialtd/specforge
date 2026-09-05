## MODIFIED Requirements

### Requirement: Unconditional Progress Layer

The Dashboard's **progress layer** SHALL always be present. It comprises the activity-log-derived views (the Today's Progress hero, the streak, and the contribution heatmap), the commit garden, and the live celebrations. No setting SHALL gate any part of it: the application SHALL NOT persist a progress-layer preference, SHALL NOT expose a control to disable it in any frontend, and SHALL NOT expose a command to read or write such a preference. The layer SHALL be computed and presented on every Dashboard render, subject only to each surface's own conditions — the commit garden's dormant and degraded states, and the viewer's `prefers-reduced-motion` setting, which remains the only suppressor of motion.

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

The activity-log-derived achievement views — the *Today's Progress Hero*'s today-flow counts (changes shipped, commits landed, tasks completed) and the *Streak and Contribution Heatmap* — SHALL count only activity that resolves to the canonical developer, per the `developer-identity` capability's query-time resolution, with author-less legacy events counted as the developer's. This personal (*Me*) resolution is unconditional: the Dashboard SHALL NOT present a control to widen these views to other authors, and SHALL NOT present a control to restrict them to any narrower window than the available history. Cross-author comparison is outside the Dashboard's concern entirely: the Dashboard SHALL NOT rank, score, or otherwise order authors against one another, so the personal frame is the only frame these views have. This prohibition governs the ordering of *authors*; it does not restrict surfaces that merely present several authors without ranking them, such as the commit garden's per-author node colouring, nor the ordering of workspaces required by the *Dashboard Includes Disabled Workspaces* requirement. The *Today's Progress Hero*'s in-flight active-change count is likewise the developer's, as specified by that requirement. These views SHALL be computed from the in-memory activity log and the shared git mining; resolving them SHALL NOT trigger a separate git-history re-mine.

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

#### Scenario: No cross-author ranking is presented

- **WHEN** the Dashboard renders for a history holding several distinct authors
- **THEN** no ranking, scoreboard, or ordered comparison of those authors against one another is shown
- **AND** the per-author surfaces that do render, such as the commit garden, distinguish authors without ordering them

### Requirement: Dashboard Section Order

The Dashboard SHALL present its sections in a fixed vertical order: the
developer profile and streak, today's progress counts, today's ships feed, the
contribution heatmap, then the analytics band. That order SHALL NOT depend on
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

#### Scenario: The terminal frontend agrees on the order

- **WHEN** the terminal frontend renders its dashboard screen
- **THEN** its ships-today section appears above its activity section

## REMOVED Requirements

### Requirement: Per-Author Leaderboard

**Reason**: The leaderboard is removed. It was the only surface that ranked authors against one another, it informed no decision, and for a single-author history it never rendered at all. Its replacement is nothing: cross-author ranking is now positively prohibited by the *Personal Progress Frame* requirement above, and the commit garden remains the only Dashboard surface that distinguishes authors — by colour, without ordering them. The named-people roster this requirement depended on is removed with it (`developer-identity`: *Named People Roster*).
