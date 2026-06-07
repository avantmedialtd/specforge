## MODIFIED Requirements

### Requirement: Equipped Badge Treatments

The Dashboard SHALL render the developer's **equipped treatment** as a finish over their **profile avatar** (the identicon). Browsing the locker of unlocked finishes and choosing which one is equipped SHALL be a **Settings** surface ("Badge finishes"), not the Dashboard — the Dashboard reflects the equipped finish but does not host the picker. Rendering an equipped treatment SHALL make no network request, and an animated finish SHALL be suppressed when the viewer's `prefers-reduced-motion` setting is active.

#### Scenario: Equipped treatment renders on the avatar

- **WHEN** a treatment is equipped
- **THEN** the developer's profile avatar renders with that finish

#### Scenario: Equipping happens in Settings

- **WHEN** the developer selects a different unlocked treatment from the Settings badge-finishes locker
- **THEN** it becomes the equipped finish
- **AND** the Dashboard renders the avatar with it

#### Scenario: Reduced motion suppresses an animated finish

- **WHEN** the viewer's `prefers-reduced-motion` setting is active
- **AND** the equipped treatment is animated
- **THEN** its motion is suppressed

### Requirement: Developer Profile Surface

The Dashboard SHALL present a developer **profile** surface identifying the canonical developer by the display name from the identity configuration and by an **avatar**. The avatar SHALL be generated locally as a deterministic identicon derived from the developer's normalised identity key, tinted from the application's existing token palette, and SHALL NOT be fetched over the network or transmit identity data off the machine. When the gamified layer is enabled and a treatment is equipped, the avatar SHALL carry that equipped treatment finish, per the *Equipped Badge Treatments* requirement. The profile surface SHALL present the developer's *Me*-scoped streak as a personal highlight alongside the avatar, retaining the encouraging zero state when the developer has no recorded activity.

#### Scenario: Profile shows the developer's name and a local avatar

- **WHEN** the profile surface renders with an identity configured
- **THEN** it shows the canonical display name
- **AND** it shows a locally-generated identicon avatar derived from the developer's identity, with no network request

#### Scenario: Profile reflects the developer's own activity

- **WHEN** the profile surface renders
- **THEN** the streak it shows is computed over the *Me*-scoped achievements

#### Scenario: Empty profile is encouraging

- **WHEN** the developer has no recorded *Me*-scoped activity
- **THEN** the profile renders an encouraging zero state rather than an error or a discouraging empty board

### Requirement: Gamification Opt-In

The gamified progress layer SHALL be gated behind a setting that is **disabled by default**. The gated layer comprises the gamified, activity-log-derived views (today's progress, streak, contribution heatmap), the live celebrations, the per-author leaderboard, and every season surface (the season home, the equipped-treatment finish on the avatar, the seasonal leaderboard, the live tier-up, and the permanent career-tier readout). When the setting is disabled, the Dashboard SHALL render only its analytics — the cross-workspace summary metrics, per-repository breakdown, git-mined activity chart, change-lifecycle metrics, and recent-activity feed — and SHALL NOT compute or present any gated section; the Settings *Badge finishes* surface SHALL likewise be hidden. Enabling the setting SHALL restore the gamified layer. The setting SHALL persist in the application's data directory.

#### Scenario: Gamification is off by default

- **WHEN** the gamification setting has never been enabled
- **THEN** the gamified layer is disabled
- **AND** the Dashboard renders only its analytics sections

#### Scenario: Enabling restores the gamified layer

- **WHEN** the gamification setting is enabled
- **THEN** the Dashboard presents the gamified layer — today's progress, streak, heatmap, the season surfaces, the leaderboard, and celebrations

#### Scenario: Disabled hides the Settings locker

- **WHEN** gamification is disabled
- **THEN** the Settings badge-finishes locker is not shown

#### Scenario: Disabled skips gamified computation

- **WHEN** gamification is disabled
- **THEN** the gamified sections are not computed for the Dashboard payload

### Requirement: Personal Gamified Frame

The gamified, activity-log-derived achievement views — the *Today's Progress Hero*'s today-flow counts (changes shipped, commits landed, tasks completed) and the *Streak and Contribution Heatmap* — SHALL count only activity that resolves to the canonical developer, per the `developer-identity` capability's query-time resolution, with author-less legacy events counted as the developer's. This personal (*Me*) resolution is unconditional: the Dashboard SHALL NOT present a control to widen these views to other authors, and SHALL NOT present a control to restrict them to a single season's window. Cross-author comparison is the concern of the per-author **Leaderboard** (and its seasonal variant); the active season's standing is the concern of the **season home** and the **seasonal leaderboard** — none of which is the personal frame. The *Today's Progress Hero*'s in-flight active-change count is likewise the developer's, as specified by that requirement. These views SHALL be computed from the in-memory activity log and the shared git mining; resolving them SHALL NOT trigger a separate git-history re-mine.

#### Scenario: Gamified views count only the developer's activity

- **WHEN** the activity log holds achievements by the developer and by other authors
- **THEN** the today-flow, streak, and heatmap views count only the achievements resolving to the developer
- **AND** the Dashboard offers no control to widen them to all authors

#### Scenario: No control to narrow the gamified views to a season

- **WHEN** the Dashboard renders its gamified frame
- **THEN** the today-flow, streak, and heatmap views cover all available history
- **AND** the Dashboard offers no lens control to restrict them to the active season's window

#### Scenario: Claiming an alias folds activity into the developer's counts

- **WHEN** activity recorded under an identity not yet claimed is excluded from the developer's counts
- **AND** that identity is added as an alias of the developer
- **THEN** the gamified views subsequently count that activity, without the activity log being rewritten

## REMOVED Requirements

### Requirement: Milestones and Badges

**Reason:** The *Milestones* panel is removed. Its thresholds are crossed off cumulative totals, so the git-history backfill silently earns every threshold the developer's past already satisfies — the panel opens pre-filled with retroactive trophies that were never felt being earned, then freezes until a distant next rung, which defeats its purpose as a sense of achievement. As UI it is a flat emoji-glyph list sorted by recency, with the timestamp-less streak badges always sinking to the bottom. The panel's one live role — hosting the seasonal **equipped treatment** finish — is re-homed onto the developer's profile avatar by the modified *Equipped Badge Treatments* requirement, so the season cosmetics survive on a live, always-visible surface rather than an inert list.
