# Number seasons from OpenSpec's launch

## Why

The dashboard's season panel labels the active season with the **raw internal
season index** — `year*12 + (month-1)` — so the current month (June 2026) reads
**"Season 24317."** That integer is a fine internal *key*: it is what makes the
season name, objective rotation, and badge treatments deterministic and
reproducible across backfilled history. But as a human label it is noise — it
has no shared zero point. Seasons should count from where the pass conceptually
begins: **OpenSpec's first release, September 2025, is Season 1.** This gives the
battle pass a legible, launch-relative number ("Season 10") without disturbing
any of the determinism the index underpins.

## What Changes

- Introduce a **season epoch** anchored to OpenSpec's first release —
  **September 2025** — as the Season 1 origin.
- Derive a **launch-relative season number** = `season_index − epoch_index + 1`,
  surfaced for every human-facing season label. September 2025 → **Season 1**;
  the current month, June 2026 → **Season 10** (the pass reads as established, ten
  seasons deep, rather than brand-new).
- The dashboard season eyebrow shows this **number** in place of the raw index.
- The number is **display-only**. The internal season index continues to drive
  the deterministic season **name**, **objective** selection, and **badge
  treatment** descriptors, so no already-unlocked treatment or generated name
  shifts. This is **not breaking** for earned content.
- Define behaviour for months **before the epoch**: the number floors so that no
  zero or negative season label can surface. No recorded activity reaches before
  September 2025 today (this repo's history begins May 2026 = Season 9), so the
  floor is a defined edge, not a live case — but it is specified, not left to
  arithmetic.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `seasons`: the *Monthly Season Model and Deterministic Naming* requirement
  gains a **launch-relative display number**, anchored to OpenSpec's first
  release (September 2025 = 1), that is distinct from the internal season index.
  The index remains the sole determinism key for names, objectives, and
  treatments; the number is purely for presentation.
- `dashboard`: the season-panel requirement is updated so the panel's label
  shows the launch-relative **season number** (e.g. "Season 10"), rather than
  leaving the eyebrow unspecified and implicitly bound to the raw index.

## Impact

- `crates/openspec-core/src/seasons.rs`: a `SEASON_EPOCH` constant
  (`season_index_for(2025, 9)` = 24308), a `season_number(index)` derivation, and
  a `number` field carried on `SeasonInfo` via `season_info`.
- `src/types.ts`: mirror the new `SeasonInfo.number` field (camelCase).
- `src/components/DashboardView.tsx`: the eyebrow renders `season.season.number`
  instead of `season.season.index`.
- Tests (`crates/openspec-core`): epoch arithmetic (Sept 2025 → 1, June 2026 →
  10) and the pre-epoch floor.
- No data migration, no persisted-state change. Treatments, names, and
  objectives are untouched — the index that generates them does not move.
