## MODIFIED Requirements

### Requirement: Today's Progress Hero

The Dashboard SHALL present a "Today's Progress" band as its topmost content, showing counts of achievements recorded for the current local calendar day aggregated across all registered workspaces. The counts SHALL be presented as a fixed left-to-right sequence ordered coarsest unit of work first — changes archived (shipped), changes created (started), commits landed, then tasks completed — so the two change-level events lead as a pair and the within-change increments follow. Each count SHALL render with an animated count-up on first render and SHALL be accompanied by a comparison to the user's recent daily average for that achievement type. When the viewer's `prefers-reduced-motion` setting is active, counts SHALL render at their final value without animation. The day boundary SHALL be the viewer's local calendar day, consistent with the commit-graph rail's day grouping.

#### Scenario: Today's counts reflect the current day

- **WHEN** the Dashboard renders
- **THEN** each Today's Progress count equals the number of achievements of that type recorded for the current local calendar day across all workspaces
- **AND** achievements recorded on prior days are excluded from the counts

#### Scenario: Counts lead with shipped

- **WHEN** the Today's Progress band renders
- **THEN** the four counts appear in the fixed left-to-right order: changes archived (shipped), changes created (started), commits landed, tasks completed

#### Scenario: Comparison to recent daily average

- **WHEN** a Today's Progress count renders
- **THEN** it shows a comparison indicator relative to the user's trailing recent-day average for that achievement type

#### Scenario: Reduced motion disables the count-up

- **WHEN** the viewer's `prefers-reduced-motion` setting is active
- **THEN** the counts render immediately at their final values without animation

#### Scenario: A day with no achievements

- **WHEN** no achievements have been recorded for the current day
- **THEN** the Today's Progress band renders an encouraging zero state
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
- **THEN** the Dashboard reveals that day's per-kind breakdown, ordered consistently with the Today's Progress band (changes shipped, changes started, commits, tasks completed)
- **AND** a day with no recorded activity reveals an explicit empty state rather than nothing

#### Scenario: Heatmap window is bounded

- **WHEN** activity exists older than the heatmap window
- **THEN** the heatmap renders only the bounded window and does not require the full history
