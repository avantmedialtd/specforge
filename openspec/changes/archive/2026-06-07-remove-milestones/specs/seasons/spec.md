## MODIFIED Requirements

### Requirement: Procedural Badge Treatments

Each battle-pass tier SHALL unlock a **badge treatment** — a rendering finish applied to the developer's **profile avatar** (the identicon) — described by a deterministic function of the `(season index, tier index)` pair. A treatment's **rarity** SHALL increase with tier index. The treatment descriptor SHALL be computed locally with no runtime network access, consistent with the local identicon; any artwork it composes SHALL be a build-time asset rather than fetched at runtime. The descriptor SHALL carry a **generator version** so that a later change to the generator does not alter the rendering of a previously unlocked treatment. A treatment SHALL be applied **as a finish over** the avatar, not as a replacement of it, so the avatar remains the developer's legible identity mark.

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

#### Scenario: Applied as a finish over the avatar

- **WHEN** a treatment is equipped
- **THEN** it renders as a finish over the developer's profile avatar rather than replacing the avatar

### Requirement: Silent Backfilled Seasons

On first observation of a git-backed workspace's history, the system SHALL reconstruct past seasons' standings over the bounded backfill window and SHALL unlock the treatments those past seasons earned into the locker **silently** — consistent with the principle that standing recovered through backfill is shown as earned but does not trigger a live celebration.

#### Scenario: Backfilled seasons unlock silently

- **WHEN** past seasons are reconstructed from history
- **THEN** the treatments their scores earned are unlocked into the locker
- **AND** no live celebration is triggered for them
