# seasons

## ADDED Requirements

### Requirement: Monthly Season Model and Deterministic Naming

The system SHALL organise gamified standings into **seasons**, each spanning exactly one calendar month in the viewer's local time zone, consistent with the commit-graph rail's day grouping. Each season SHALL have a stable integer **season index** derived solely from its calendar year and month, so the same month always resolves to the same index. Each season SHALL have a **name** generated deterministically from its season index, requiring no per-season authoring. The current season SHALL be the one whose month contains the present local day; its **window** SHALL be the half-open interval from the first instant of its month to the first instant of the next month.

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

### Requirement: Two-Track Progression — Resetting Season, Permanent Career

The system SHALL maintain two parallel progression tracks. The **seasonal** track — the season score, its band and tier, the active objectives, and the battle-pass track — SHALL reset at each season boundary, beginning every season at zero. The **career** track — a permanent tier derived from lifetime cumulative totals — SHALL only ever rise and SHALL NOT be demoted by a season boundary or by a low-output season. This never-demote guarantee covers **organic play**: because lifetime totals only ever grow against a fixed set of career thresholds, the derived tier never decreases over time. A **deliberate retune of the career thresholds** is a rebalance, not play: the tier is recomputed against the new thresholds and MAY change, including downward, and this does NOT violate the never-demote guarantee. The current **streak** SHALL be treated as a career line and SHALL survive season boundaries; a season boundary alone SHALL NOT reset or break the streak.

#### Scenario: Seasonal standings reset at the boundary

- **WHEN** a new season begins
- **THEN** the season score, band and tier, objectives, and battle-pass track restart at zero

#### Scenario: The career tier never demotes through play

- **WHEN** a season ends with low output
- **THEN** the career tier does not decrease

#### Scenario: A threshold rebalance recomputes the career tier

- **WHEN** the career thresholds are deliberately retuned
- **THEN** the career tier is recomputed against the new thresholds
- **AND** it MAY change, including downward, since a rebalance is not organic play and is outside the never-demote guarantee

#### Scenario: The streak survives the boundary

- **WHEN** a season boundary is crossed on a day that continues an active streak
- **THEN** the streak is not reset or broken by the boundary alone

### Requirement: Season Score Derivation

The season score SHALL be a derivation over existing data within the active season's window, resolved to the local developer (Me scope): a weighted sum of the developer's recorded achievement events (a ship, an artifact reached, a task completed with its magnitude, a change created, and active-day credit) together with the developer's authored commits mined from git. The system SHALL NOT record any new event kind for seasons; the score SHALL be recomputed from the append-only activity log and the existing commit mining. Within a single season the score SHALL be monotonic non-decreasing as qualifying activity accrues.

#### Scenario: Score derives from existing sources

- **WHEN** the season score is computed
- **THEN** it is a weighted sum over the Me-scoped activity-log events and the Me-authored commits within the season window
- **AND** no new event kind is recorded

#### Scenario: Score is Me-scoped

- **WHEN** the activity log holds events by the developer and by other authors
- **THEN** only events resolving to the developer contribute to the season score

#### Scenario: Score only rises within a season

- **WHEN** further qualifying activity accrues within the season
- **THEN** the season score does not decrease

### Requirement: Battle-Pass Tier Ladder and Named Bands

The season score SHALL drive a battle-pass **tier ladder**: a sequence of fine-grained tiers grouped into named **bands**, such that a single score reads both as a precise tier (with progress toward the next tier) and as a coarse band identity. The system SHALL expose the current tier, the current band, and the gap, in score, to the next tier.

#### Scenario: Score maps to a tier and a band

- **WHEN** the season score reaches a tier's threshold
- **THEN** the current tier and its enclosing band advance accordingly

#### Scenario: Gap to the next tier is exposed

- **WHEN** the ladder is presented
- **THEN** it shows the remaining score needed to reach the next tier

#### Scenario: Bands group tiers

- **WHEN** tiers are presented
- **THEN** consecutive tiers are grouped under named bands so the standing reads as both a tier and a band

### Requirement: Adaptive Pacing with an Overflow Lane

The total season score required to complete the battle-pass track SHALL scale to the developer's recent trailing baseline (the recent-daily-average already used for the Today's-Progress comparison) over the season's days, so the track is a stretch rather than a fixed wall. The baseline's influence SHALL be **clamped** between a floor and a ceiling, so the track is never trivially completed nor made impossible. Beyond the final tier the track SHALL provide an unbounded **overflow lane** of further tiers, so output past completion is not wasted.

#### Scenario: Pacing scales to the baseline

- **WHEN** two developers with different sustained output each play a season
- **THEN** each one's required completion total reflects their own trailing baseline rather than a single global threshold

#### Scenario: Baseline influence is clamped

- **WHEN** a developer's trailing baseline is extremely low or extremely high
- **THEN** the required completion total is bounded by a floor and a ceiling rather than becoming trivial or impossible

#### Scenario: Overflow past completion

- **WHEN** the season score exceeds the final tier's threshold
- **THEN** additional overflow tiers continue to advance

### Requirement: Rotating Generated Objectives

Each season SHALL present a small set of **objectives** generated from reusable archetypes (such as volume, cadence, streak, burst, breadth, full-lifecycle finish, and comeback), each derivable from existing achievement data. The objectives for a season SHALL be selected deterministically from the season index and SHALL rotate so that the same archetype does not recur in consecutive seasons. Each objective's threshold SHALL be scaled to the developer's trailing baseline. Objective progress SHALL be derived from the season-window activity, and completing an objective SHALL grant bonus season score. No objective SHALL require hand-authoring per season.

#### Scenario: Objectives are generated and rotate

- **WHEN** consecutive seasons present objectives
- **THEN** each season's set is selected deterministically from its season index
- **AND** an archetype does not recur in back-to-back seasons

#### Scenario: Thresholds adapt to the developer

- **WHEN** an objective is presented
- **THEN** its threshold is scaled to the developer's trailing baseline rather than a fixed global value

#### Scenario: Completion grants bonus score

- **WHEN** an objective's derived progress reaches its threshold
- **THEN** it is marked complete
- **AND** bonus season score is granted

### Requirement: Procedural Badge Treatments

Each battle-pass tier SHALL unlock a **badge treatment** — a rendering finish applied to the developer's earned milestone badges — described by a deterministic function of the `(season index, tier index)` pair. A treatment's **rarity** SHALL increase with tier index. The treatment descriptor SHALL be computed locally with no runtime network access, consistent with the local identicon; any artwork it composes SHALL be a build-time asset rather than fetched at runtime. The descriptor SHALL carry a **generator version** so that a later change to the generator does not alter the rendering of a previously unlocked treatment. A treatment SHALL be applied **to** earned badges as a finish, not as a one-per-badge replacement, so the badge set is not exhausted.

#### Scenario: Treatment is deterministic per season and tier

- **WHEN** the same `(season index, tier index)` pair is evaluated
- **THEN** it yields the same treatment descriptor

#### Scenario: Rarity rises with tier

- **WHEN** treatments at higher tiers are compared with those at lower tiers
- **THEN** the higher-tier treatments are of greater rarity

#### Scenario: No runtime network

- **WHEN** a treatment is resolved and rendered
- **THEN** no network request is made
- **AND** any artwork used is a bundled build-time asset

#### Scenario: Stable across generator changes

- **WHEN** the generator changes in a later version
- **THEN** a previously unlocked treatment still resolves to its original rendering via its recorded generator version

#### Scenario: Applied as a finish to earned badges

- **WHEN** a treatment is equipped
- **THEN** it renders as a finish over the developer's earned milestone badges rather than replacing a badge

### Requirement: Treatment Locker, Equipping, and Soft-FOMO Vault

The system SHALL persist a **locker** of unlocked treatments and the developer's **equipped** selection in the application's data directory, alongside other application settings, and SHALL NOT write this state into any workspace's `openspec/` tree. Unlocking SHALL be monotonic: a treatment once unlocked SHALL remain owned and SHALL NOT be revoked by a later recompute. A treatment SHALL be **earnable** when it belongs to the current season or to the current **vault** — a deterministic rotation of past seasons' treatments — so that a treatment missed in its original season MAY be earned later (soft FOMO), while treatments already earned are never lost.

#### Scenario: Locker persists in app-data only

- **WHEN** treatments are unlocked or a treatment is equipped
- **THEN** the locker and equipped selection persist in the application's data directory
- **AND** no file under any workspace's `openspec/` tree is created or modified

#### Scenario: Unlocking is monotonic

- **WHEN** standings are recomputed after an unlock
- **THEN** previously unlocked treatments remain owned

#### Scenario: The vault makes missed treatments re-earnable

- **WHEN** a treatment was not earned in its original season
- **AND** it is in the current vault rotation
- **THEN** it is earnable in the current season

#### Scenario: Earned treatments are never lost

- **WHEN** a season ends
- **THEN** treatments already unlocked remain in the locker

### Requirement: Silent Backfilled Seasons

On first observation of a git-backed workspace's history, the system SHALL reconstruct past seasons' standings over the bounded backfill window and SHALL unlock the treatments those past seasons earned into the locker **silently** — shown as earned without triggering any live celebration — consistent with the existing rule that backfilled milestones are shown as earned but do not celebrate.

#### Scenario: Backfilled seasons unlock silently

- **WHEN** past seasons are reconstructed from history
- **THEN** the treatments their scores earned are unlocked into the locker
- **AND** no live celebration is triggered for them

#### Scenario: Backfill is bounded

- **WHEN** history exists older than the backfill window
- **THEN** seasons outside the window are not reconstructed

### Requirement: Season Rollover and Recap

When a season boundary is crossed — detected at launch and while the application is running — the system SHALL mint a **recap** of the just-ended season, synthesised from that season's window of activity (such as changes shipped, best streak, band reached, objectives completed, and treatments unlocked), and SHALL surface it once. Seasons that ended before the developer last saw a recap, and backfilled historical seasons, SHALL be available in history **without** a live celebration. The system SHALL persist enough state — the last recapped season — to avoid re-presenting a recap, and at first launch SHALL initialise that state to the current season so historical months do not each fire a recap.

#### Scenario: A recap is minted at rollover

- **WHEN** the active season index advances past the last recapped season
- **THEN** a recap of the just-ended season is synthesised from its window and surfaced once

#### Scenario: A recap is not repeated

- **WHEN** a recap has already been surfaced for a season
- **THEN** it is not surfaced again

#### Scenario: First launch does not spam recaps

- **WHEN** the application first backfills many historical seasons
- **THEN** it does not surface a recap for each historical month

### Requirement: Read-Only and Offline Operation

All season computation SHALL be read-only with respect to workspaces and git: it SHALL NOT edit a spec, toggle a task, move a change, or run any git operation that changes history or working-tree state, and it SHALL make no network request at runtime. The only state seasons persist SHALL be the treatment locker, the equipped selection, and rollover bookkeeping, all in the application's data directory.

#### Scenario: No workspace or git mutation

- **WHEN** season standings are computed or a treatment is equipped
- **THEN** no spec, task, change, or git state is modified

#### Scenario: No runtime network

- **WHEN** any season feature operates
- **THEN** no network request is made
