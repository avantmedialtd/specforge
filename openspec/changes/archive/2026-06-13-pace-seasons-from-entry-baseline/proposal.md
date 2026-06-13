# Pace Seasons From the Entry Baseline

## Why

A season's completion goal climbs **mid-season**, observed in dogfooding ("the goal of the current season seems to increase mid-season").

The goal is `target_total(baseline, days)` (`crates/openspec-core/src/seasons.rs`), the total score that completes the 30-tier battle pass; each tier line is `total / 30`. Its only moving input is `baseline` — the developer's **trailing-30-active-day** average (`crates/openspec-core/src/dashboard.rs:441`, fed in at `crates/specforge/src/commands.rs:501`). That average excludes only **today**, not the current season, and a calendar month can't exceed 30 active days — so *every day of the current season except today feeds the baseline that sets the current season's bar*. As in-season output rises, the baseline rises, the goal rises, and the bar recedes.

The consequences go past mild annoyance:

- **The visible standing can demote.** The season score is guaranteed monotonic, but `tier = score / per_tier`, and `per_tier` can rise faster than the score — so a developer's band can slip mid-season even as their score grows. This contradicts the never-demote instinct the product spells out for the career track and values everywhere.
- **Objective thresholds creep too.** `objective_target` (`seasons.rs:448`) pulls the same baseline, so "Complete N tasks" inflates N as you complete tasks.
- **Finished-season recaps drift.** The rollover recap reuses the *live* baseline (`commands.rs:592`), so a past season's recorded band is graded against *today's* form and changes every day.

## What Changes

Sample the pacing baseline from the window **immediately preceding the season** (the developer's entry form), hold it **fixed for the season's duration**, and re-sample only at season boundaries.

- The completion total and every tier line become **fixed for the season** — the bar stops drifting.
- With a fixed target and a monotonic score, tier and band become **monotonic within a season**: no mid-season demotion.
- Objective thresholds, riding the same baseline, **freeze for the season** as well.
- The rollover recap grades a finished season against **its own** entry baseline, so past standings are stable.

This needs **no core API change**: `compute_season` / `target_total` / `objective_target` / `season_recap` already take `baseline` by value — only the value passed changes, plus a generalized "trailing-active-days-before-anchor" averaging helper. The Today's-Progress tile keeps its live (anchored-at-today) baseline. No new persisted state: the pre-season window `[season start − 30 active days, season start)` is derived from history that already lives in the 371-day day-axis.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `seasons`: the pacing baseline is sourced from the pre-season window and fixed for the season's duration (re-sampled only at boundaries); the battle-pass tier and band are monotonic non-decreasing within a season; objective thresholds are fixed for the season; and a rollover recap's standing is graded against that season's entry baseline rather than the developer's current form.

## Impact

- `crates/openspec-core/src/dashboard.rs` — generalize the trailing-active-day average to anchor strictly before an arbitrary day (today-tile anchors at today; season anchors at the season's first day).
- `crates/specforge/src/commands.rs` — build a pre-season `SeasonBaseline` anchored at the active season's first day and pass it to `compute_season`; do the same for the recap's `prev` season.
- `crates/openspec-core` tests — cover the anchored average, fixed-target-within-season, no-within-season-demotion, and recap-stability.
- No frontend / IPC type changes: `SeasonBaseline` and `SeasonStanding` shapes are unchanged.
- Read-only and offline invariants preserved; no new persisted state.
