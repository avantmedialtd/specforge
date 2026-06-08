## MODIFIED Requirements

### Requirement: Monthly Season Model and Deterministic Naming

The system SHALL organise gamified standings into **seasons**, each spanning exactly one calendar month in the viewer's local time zone, consistent with the commit-graph rail's day grouping. Each season SHALL have a stable integer **season index** derived solely from its calendar year and month, so the same month always resolves to the same index. Each season SHALL have a **name** generated deterministically from its season index, requiring no per-season authoring. Each season SHALL also have a **launch-relative number** for presentation, anchored so that **OpenSpec's first release — September 2025 — is Season 1**, computed as the season index's offset from that epoch plus one. This number is display-only: it SHALL NOT influence the season index, name, objectives, or treatments, which remain derived solely from the index. The number SHALL be floored so that a season at or before the epoch never presents a zero or negative value. The current season SHALL be the one whose month contains the present local day; its **window** SHALL be the half-open interval from the first instant of its month to the first instant of the next month.

#### Scenario: A season spans a calendar month

- **WHEN** the current local date falls within a given calendar month
- **THEN** the active season's window is that month's first instant (inclusive) to the next month's first instant (exclusive)

#### Scenario: The season index is stable

- **WHEN** the same calendar month is evaluated on different launches
- **THEN** it resolves to the same season index
- **AND** the same generated season name

#### Scenario: The season name needs no authoring

- **WHEN** a season is presented
- **THEN** its name is generated deterministically from the season index with no hand-authored per-season content

#### Scenario: The season number counts from OpenSpec's launch

- **WHEN** the season for September 2025 is presented
- **THEN** its launch-relative number is 1
- **AND** the season for June 2026 presents as number 10

#### Scenario: The number is presentation-only and floored

- **WHEN** a season's launch-relative number is derived
- **THEN** it does not alter the season index or the deterministically generated name
- **AND** a season at or before the September 2025 epoch presents a number no lower than 1
