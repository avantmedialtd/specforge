## ADDED Requirements

### Requirement: Today's Progress Hero

The Dashboard SHALL present a "Today's Progress" band as its topmost content, showing counts of achievements recorded for the current local calendar day aggregated across all registered workspaces: tasks completed, changes archived (shipped), commits landed, and changes created. Each count SHALL render with an animated count-up on first render and SHALL be accompanied by a comparison to the user's recent daily average for that achievement type. When the viewer's `prefers-reduced-motion` setting is active, counts SHALL render at their final value without animation. The day boundary SHALL be the viewer's local calendar day, consistent with the commit-graph rail's day grouping.

#### Scenario: Today's counts reflect the current day

- **WHEN** the Dashboard renders
- **THEN** each Today's Progress count equals the number of achievements of that type recorded for the current local calendar day across all workspaces
- **AND** achievements recorded on prior days are excluded from the counts

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
- **THEN** the Dashboard reveals that day's per-kind breakdown (tasks completed, changes shipped, commits, changes started)
- **AND** a day with no recorded activity reveals an explicit empty state rather than nothing

#### Scenario: Heatmap window is bounded

- **WHEN** activity exists older than the heatmap window
- **THEN** the heatmap renders only the bounded window and does not require the full history

### Requirement: Milestones and Badges

The Dashboard SHALL present milestone achievements crossed at defined cumulative thresholds — including the first change shipped, task-completion thresholds, change-shipped thresholds, and streak-length thresholds — derived from the activity log's cumulative totals, and SHALL show a list of the most recently crossed milestones. A milestone whose threshold was already satisfied by history recovered through backfill SHALL be shown as earned but SHALL NOT trigger a live celebration.

#### Scenario: A milestone is crossed

- **WHEN** a cumulative total reaches a defined milestone threshold
- **THEN** that milestone is marked earned
- **AND** it appears in the recent-milestones list

#### Scenario: Backfilled milestones are silent

- **WHEN** a milestone threshold was already satisfied by backfilled history at first launch
- **THEN** the milestone is shown as earned
- **AND** no live celebration is triggered for it

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

## MODIFIED Requirements

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
