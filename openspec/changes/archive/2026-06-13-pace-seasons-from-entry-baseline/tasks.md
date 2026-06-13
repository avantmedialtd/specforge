# Tasks

## 1. Generalize the trailing-active-day average

- [x] 1.1 In `crates/openspec-core/src/dashboard.rs`, refactor `trailing_avg_centi` (and `commits_trailing_avg_centi`) to take an **anchor day** and average the 30 most-recent active days with `day < anchor`, instead of hardcoding `!= today`. Keep the today-tile callers passing `anchor = today` (behavior unchanged).
- [x] 1.2 Expose a small `season_baseline(...)` helper (or reuse the per-day maps `compute_progress` builds) that returns a `SeasonBaseline` anchored at a given season-start day, so both the active season and the recap can build a pre-season baseline from the same code.
- [x] 1.3 Unit-test the anchored average: a fixed day-set yields the today-anchored mean vs. a season-start-anchored mean that excludes the in-season days.

## 2. Pace the active season from the entry baseline

- [x] 2.1 In `crates/specforge/src/commands.rs`, build a pre-season `SeasonBaseline` anchored at the active season's first day (`format!("{:04}-{:02}-01", info.year, info.month)`) and pass **it** to `compute_season` instead of the live `base_progress`-derived baseline.
- [x] 2.2 Leave the Today's-Progress tile on its live (anchored-at-today) baseline — the two baselines now answer different questions.

## 3. Grade the recap against the season's own entry baseline

- [x] 3.1 In the rollover-recap branch (`commands.rs:592`), build a baseline anchored at `prev`'s first day and pass it to `season_recap`, replacing the reused live `baseline`.

## 4. Lock the guarantees with tests

- [x] 4.1 In `crates/openspec-core/src/seasons.rs` tests, assert the completion total is **fixed** when only in-season activity grows (target unchanged across rising in-season events for a fixed entry baseline).
- [x] 4.2 Assert the tier/band is **monotonic non-decreasing within a season** given a fixed target and a monotonic score (no demotion).
- [x] 4.3 Assert objective targets are unchanged as in-season progress rises (only `progress` advances).
- [x] 4.4 Assert a recap's band/tier depends only on the season's entry baseline, not on later activity.
- [x] 4.5 Cover the sparse/zero pre-season case: the floor governs `target_total` and objective targets.

## 5. Verify

- [x] 5.1 `cargo test -p openspec-core` (seasons + dashboard) green.
- [x] 5.2 `bun run build` (tsc) green — confirm no `SeasonBaseline` / `SeasonStanding` IPC shape drift (expected: none).
- [x] 5.3 Confirm the bar holds steady. The drift is a *cross-day* phenomenon (today is always excluded from the live average), so it isn't reproducible inside one dev session; the guarantee is instead locked by `season_baseline_ignores_activity_from_the_anchor_day_onward` (the entry baseline is provably invariant to on/after-anchor activity) plus the fixed-target / no-demotion seasons tests. `cargo check -p specforge` green, so the dashboard still builds and renders.
