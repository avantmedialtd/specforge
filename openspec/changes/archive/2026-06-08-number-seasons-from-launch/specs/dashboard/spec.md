## MODIFIED Requirements

### Requirement: Season Home on the Profile Band

The Dashboard's profile band SHALL present a **season home** for the active season: its **launch-relative season number** and generated name, a countdown to the season's end, the current band and tier with the gap to the next tier, the battle-pass track with the next unlock previewed, the active objectives with their progress, and the developer's equipped treatment. The displayed season number SHALL be the launch-relative value anchored at OpenSpec's first release (September 2025 = Season 1), not the raw internal season index. The season home SHALL be Me-scoped and SHALL retain an encouraging zero state when the developer has no season activity yet.

#### Scenario: Season home shows the active season

- **WHEN** the profile band renders
- **THEN** it shows the launch-relative season number and the season name, the end countdown, the current band and tier with the gap to the next tier, the battle-pass track with the next unlock previewed, the active objectives with their progress, and the equipped treatment

#### Scenario: The label uses the launch-relative number, not the index

- **WHEN** the season home labels the active season in June 2026
- **THEN** it presents the launch-relative number (Season 10)
- **AND** it does not present the raw internal season index (24317)

#### Scenario: Encouraging zero state

- **WHEN** the developer has no recorded activity in the active season
- **THEN** the season home renders an encouraging zero state rather than an error or a discouraging empty board
