## MODIFIED Requirements

### Requirement: Adaptive Pacing with an Overflow Lane

The total season score required to complete the battle-pass track SHALL scale to the developer's **entry baseline** — a recent-daily-average derived from the developer's activity in the window **immediately preceding the season's start**, computed by the same trailing-active-day method used for the Today's-Progress comparison but anchored at the season boundary rather than the present day — over the season's days, so the track is a stretch rather than a fixed wall. Because the baseline is sampled from before the season, the completion total SHALL be **fixed for the duration of the season** and SHALL NOT change as the developer's in-season output accrues; it SHALL be re-sampled only when a new season begins. The baseline's influence SHALL be **clamped** between a floor and a ceiling, so the track is never trivially completed nor made impossible — including when little or no pre-season history exists, in which case the floor governs. Beyond the final tier the track SHALL provide an unbounded **overflow lane** of further tiers, so output past completion is not wasted.

#### Scenario: Pacing scales to the entry baseline

- **WHEN** two developers with different sustained output each enter a season
- **THEN** each one's required completion total reflects their own pre-season trailing baseline rather than a single global threshold

#### Scenario: The completion total is fixed within a season

- **WHEN** the developer's output rises during the active season
- **THEN** the season's completion total does not change
- **AND** the baseline is re-sampled only when the next season begins

#### Scenario: Baseline influence is clamped

- **WHEN** a developer's pre-season baseline is extremely low or extremely high, or no pre-season history exists
- **THEN** the required completion total is bounded by a floor and a ceiling rather than becoming trivial or impossible

#### Scenario: Overflow past completion

- **WHEN** the season score exceeds the final tier's threshold
- **THEN** additional overflow tiers continue to advance

### Requirement: Battle-Pass Tier Ladder and Named Bands

The season score SHALL drive a battle-pass **tier ladder**: a sequence of fine-grained tiers grouped into named **bands**, such that a single score reads both as a precise tier (with progress toward the next tier) and as a coarse band identity. The system SHALL expose the current tier, the current band, and the gap, in score, to the next tier. Because the completion total is fixed for the duration of the season and the season score is monotonic non-decreasing within that season, the current tier and band SHALL likewise be **monotonic non-decreasing within a season**: a developer's standing SHALL NOT demote mid-season as further qualifying activity accrues.

#### Scenario: Score maps to a tier and a band

- **WHEN** the season score reaches a tier's threshold
- **THEN** the current tier and its enclosing band advance accordingly

#### Scenario: Gap to the next tier is exposed

- **WHEN** the ladder is presented
- **THEN** it shows the remaining score needed to reach the next tier

#### Scenario: Bands group tiers

- **WHEN** tiers are presented
- **THEN** consecutive tiers are grouped under named bands so the standing reads as both a tier and a band

#### Scenario: The standing does not demote within a season

- **WHEN** further qualifying activity accrues within the active season
- **THEN** the current tier and band do not decrease

### Requirement: Rotating Generated Objectives

Each season SHALL present a small set of **objectives** generated from reusable archetypes (such as volume, cadence, streak, burst, breadth, full-lifecycle finish, and comeback), each derivable from existing achievement data. The objectives for a season SHALL be selected deterministically from the season index and SHALL rotate so that the same archetype does not recur in consecutive seasons. Each objective's threshold SHALL be scaled to the developer's **entry baseline** — the same pre-season trailing baseline that paces the battle-pass track — and SHALL be **fixed for the duration of the season**, not re-scaled as in-season activity accrues. Objective progress SHALL be derived from the season-window activity, and completing an objective SHALL grant bonus season score. No objective SHALL require hand-authoring per season.

#### Scenario: Objectives are generated and rotate

- **WHEN** consecutive seasons present objectives
- **THEN** each season's set is selected deterministically from its season index
- **AND** an archetype does not recur in back-to-back seasons

#### Scenario: Thresholds adapt to the developer

- **WHEN** an objective is presented
- **THEN** its threshold is scaled to the developer's pre-season entry baseline rather than a fixed global value

#### Scenario: Objective thresholds do not move mid-season

- **WHEN** the developer's in-season output rises
- **THEN** each objective's target is unchanged
- **AND** only its derived progress advances

#### Scenario: Completion grants bonus score

- **WHEN** an objective's derived progress reaches its threshold
- **THEN** it is marked complete
- **AND** bonus season score is granted

### Requirement: Season Rollover and Recap

When a season boundary is crossed — detected at launch and while the application is running — the system SHALL mint a **recap** of the just-ended season, synthesised from that season's window of activity (such as changes shipped, best streak, band reached, objectives completed, and treatments unlocked), and SHALL surface it once. A recap's band and tier SHALL be graded against that season's **own entry baseline** — the trailing baseline preceding that season's start — so a finished season's standing is stable and does not drift as the developer's later output changes. Seasons that ended before the developer last saw a recap, and backfilled historical seasons, SHALL be available in history **without** a live celebration. The system SHALL persist enough state — the last recapped season — to avoid re-presenting a recap, and at first launch SHALL initialise that state to the current season so historical months do not each fire a recap.

#### Scenario: A recap is minted at rollover

- **WHEN** the active season index advances past the last recapped season
- **THEN** a recap of the just-ended season is synthesised from its window and surfaced once

#### Scenario: A recap's standing reflects the season's entry baseline

- **WHEN** a recap is minted for a finished season
- **THEN** its band and tier reflect that season's entry baseline
- **AND** they do not change as the developer's later activity changes

#### Scenario: A recap is not repeated

- **WHEN** a recap has already been surfaced for a season
- **THEN** it is not surfaced again

#### Scenario: First launch does not spam recaps

- **WHEN** the application first backfills many historical seasons
- **THEN** it does not surface a recap for each historical month
