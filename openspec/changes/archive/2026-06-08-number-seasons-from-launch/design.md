# Design — Number seasons from OpenSpec's launch

## Context

The seasons engine (`crates/openspec-core/src/seasons.rs`) keys everything off a
single integer **season index**: `season_index_for(year, month) = year*12 +
(month-1)`. That index seeds the deterministic generators — `season_name(index)`
(splitmix64 over the index), objective selection, and the badge-treatment
descriptors whose ids bake the `(index, tier)` pair plus `GENERATOR_VERSION` so a
previously unlocked treatment keeps its rendering. The index is therefore a
**determinism key**, not a label.

But it is currently *used* as a label: `DashboardView.tsx:329` renders
`Season {season.season.index}`, so June 2026 reads "Season 24317." There is no
shared origin. The proposal fixes the origin to **OpenSpec's first release,
September 2025 = Season 1**, and derives a launch-relative number for display
while leaving the index — and thus all generated content — untouched.

`SeasonInfo` (the serialized season header consumed by the dashboard) currently
carries `index, name, year, month, start_ts, end_ts`. The dashboard's
season-panel spec lists the panel's contents (name, countdown, band/tier, …) but
is silent on a number, so the eyebrow's use of the raw index is unspecified
today.

## Goals / Non-Goals

**Goals:**
- Display a launch-relative season number anchored at September 2025 = Season 1.
- Keep the absolute season index as the sole determinism key, so no generated
  name, objective, or unlocked treatment shifts.
- One source of truth for the epoch and the derivation, consumable by every
  season surface (panel today; recap/history later).
- A defined, non-negative floor for pre-epoch months.

**Non-Goals:**
- Re-numbering or re-basing the internal index.
- Making the epoch user-configurable (it is a fixed historical fact).
- Changing scoring, ladders, objectives, treatments, persistence, or the
  monthly-season cadence.
- Building a season-history browser (the number is shaped to support one later,
  but none is added here).

## Decisions

### 1. A display-only epoch, not a re-anchored index

Derive a separate **number** = `index − SEASON_EPOCH + 1` and leave `index`
alone.

*Alternative considered:* re-base the index itself so September 2025 → 1. Rejected
— the index seeds `season_name`, objective rotation, and treatment ids
(`(index, tier)` + `GENERATOR_VERSION`). Shifting it would change every
generated season name and alter the identity of **already-unlocked treatments**,
which the engine explicitly promises never to do. The number must be a *view* of
the index, exactly as `name` already is.

### 2. Compute in `openspec-core`, carry on `SeasonInfo`

Add `pub number: i64` to `SeasonInfo`, populated in `season_info()` beside
`name`, via a pure `pub fn season_number(index: i64) -> i64`. The epoch is a
single constant: `const SEASON_EPOCH: i64 = season_index_for(2025, 9);` (= 24308),
expressed through the existing function so it reads as "September 2025" rather
than a magic literal.

*Alternative considered:* compute `index − EPOCH + 1` inline in
`DashboardView.tsx`. Rejected — it scatters the epoch into the frontend and
invites drift the moment a second surface (recap, rollover, history,
backfilled-season list) also needs the number. Season identity belongs to the
season model; the number is a pure function of the index, so it lives next to
`name`. The frontend just renders `season.season.number`.

### 3. Pre-epoch floor: clamp the displayed number to ≥ 1

`season_number(index) = (index − SEASON_EPOCH + 1).max(1)`. Months before the
epoch never yield a zero or negative label.

*Why a clamp is safe here:* no recorded activity predates the epoch (this repo's
history begins May 2026 = Season 9) and the dashboard panel only ever renders the
**current** season, which is always ≥ Season 9. The floor is defensive, not a
live path.

*Alternative considered:* represent pre-epoch as `Option`/“Preseason”. Deferred —
it adds branching to every consumer for a case no data reaches. Revisit only if a
future history view renders pre-epoch seasons, where a clamp-to-1 collision with
the real Season 1 would actually be visible.

### 4. No persistence, no migration

`number` is recomputed on every load like `name`. The treatment locker keys off
`(index, tier)` + `GENERATOR_VERSION` and is untouched; no stored value
references the old "Season 24317" label. Rollback is a plain revert.

## Risks / Trade-offs

- **The current month visibly jumps from "Season 24317" to "Season 10."** →
  Intended — that *is* the change. Nothing persisted referenced the old number,
  so nothing breaks.
- **The epoch is a product fact living in code.** If "OpenSpec's first release"
  is ever restated, the anchor must move. → It is one named constant derived via
  `season_index_for(2025, 9)` with an explanatory comment — a one-line edit, and
  deliberately not a runtime setting so the historical anchor stays fixed.
- **Two integers on a season (`index` vs `number`) invite mis-use** (a future
  surface re-displaying the raw index). → The `dashboard` spec pins the label to
  the number; the field name + doc comment state that `index` is determinism-only.
- **Clamp-to-1 collides pre-epoch months with the real Season 1.** → Harmless
  given no pre-epoch data and a current-season-only panel; flagged for revisit if
  a history surface ever needs it.

## Migration Plan

None. Pure derivation over existing data; no schema, no persisted state, no
backfill. Deploy = ship the build; rollback = revert the commit.

## Open Questions

- Should pre-epoch seasons eventually render as "Preseason" instead of a clamped
  Season 1? Deferred until a surface actually displays pre-epoch seasons.
- Should the launch-relative number also title the **recap** ("Season 10 recap")
  when rollover surfaces one? Plausibly trivial once `number` exists, but out of
  scope for this change unless it falls out for free.
