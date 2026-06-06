# dashboard

## ADDED Requirements

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

### Requirement: Season Lens (This Season / All Time)

The Dashboard SHALL provide a **lens** control with two settings, *This Season* and *All Time*, that qualifies the activity-log-derived views (the today, streak, heatmap, and milestone views) by restricting them to the active season's window or to all available history. The lens SHALL compose with the existing *Me / Everyone* scope and SHALL recompute from the in-memory log without re-mining git.

#### Scenario: This Season restricts to the season window

- **WHEN** the lens is *This Season*
- **THEN** the qualified views count only achievements within the active season's window

#### Scenario: All Time shows full history

- **WHEN** the lens is *All Time*
- **THEN** the qualified views count achievements across the available history

#### Scenario: Lens composes with scope without re-mining

- **WHEN** the lens or the scope changes
- **THEN** the views recompute from the in-memory log
- **AND** git history mining is not re-run for the change

### Requirement: Equipped Badge Treatments

The Dashboard SHALL render the developer's **equipped treatment** as a finish over their earned milestone badges, and SHALL allow choosing which unlocked treatment is equipped from the locker. Rendering an equipped treatment SHALL make no network request, and an animated finish SHALL be suppressed when the viewer's `prefers-reduced-motion` setting is active.

#### Scenario: Equipped treatment renders on badges

- **WHEN** a treatment is equipped
- **THEN** the developer's earned milestone badges render with that finish

#### Scenario: Equipping is chosen from the locker

- **WHEN** the developer selects a different unlocked treatment
- **THEN** it becomes the equipped finish

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
