## MODIFIED Requirements

### Requirement: Gamification Opt-In

The gamified progress layer SHALL be gated behind a setting that is **disabled by default**. The gated layer comprises the gamified, activity-log-derived views (today's progress, streak, contribution heatmap), the commit garden, the live celebrations, the per-author leaderboard, and every season surface (the season home, the equipped-treatment finish on the avatar, the seasonal leaderboard, the live tier-up, and the permanent career-tier readout). When the setting is disabled, the Dashboard SHALL render only its analytics — the cross-workspace summary metrics, per-repository breakdown, git-mined activity chart, change-lifecycle metrics, and recent-activity feed — and SHALL NOT compute or present any gated section; the Settings *Badge finishes* surface SHALL likewise be hidden. Enabling the setting SHALL restore the gamified layer. The setting SHALL persist in the application's data directory.

#### Scenario: Gamification is off by default

- **WHEN** the gamification setting has never been enabled
- **THEN** the gamified layer is disabled
- **AND** the Dashboard renders only its analytics sections
- **AND** the commit garden is not shown

#### Scenario: Enabling restores the gamified layer

- **WHEN** the gamification setting is enabled
- **THEN** the Dashboard presents the gamified layer — today's progress, streak, heatmap, the season surfaces, the leaderboard, celebrations, and the commit garden

#### Scenario: Disabled hides the Settings locker

- **WHEN** gamification is disabled
- **THEN** the Settings badge-finishes locker is not shown

#### Scenario: Disabled skips gamified computation

- **WHEN** gamification is disabled
- **THEN** the gamified sections are not computed for the Dashboard payload
- **AND** the commit garden data is not computed
