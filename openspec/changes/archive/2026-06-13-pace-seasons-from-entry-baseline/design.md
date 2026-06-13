## Context

The seasonal battle pass paces its 30-tier track to the developer's recent output: `target_total(baseline, days)` (`crates/openspec-core/src/seasons.rs:335`) returns the score that completes the track, and each tier line is `per_tier = total / 30` (`seasons.rs:362`). `days` is fixed for the month, so the only moving input is `baseline`.

`baseline` is a `SeasonBaseline { ships_per_day, tasks_per_day, commits_per_day }` built at `crates/specforge/src/commands.rs:501` from `trailing_avg_centi` (`crates/openspec-core/src/dashboard.rs:441`): the mean over the **30 most-recent active days** on the 371-day `day_axis` (`crates/openspec-core/src/activity_log.rs:317`), **excluding today**. Because the axis ends at today, "excluding today" is identical to "days strictly before today," and a calendar month never exceeds 30 active days — so every day of the current season except today is inside the window that sets the current season's bar.

The feedback loop:

```
ship today → season score ▲ (monotonic)
   → tomorrow "today" rolls over → yesterday enters the trailing-30 window
   → ships/tasks/commits per-day ▲ → target_total ▲ → per_tier ▲ → every tier line ▲
   → the bar recedes (score and target rise together)
```

Three downstream effects: the visible tier/band can **demote** mid-season (score is monotonic but `score / per_tier` is not); objective thresholds inflate (`objective_target`, `seasons.rs:448`, shares the baseline); and the rollover recap reuses the *live* baseline (`commands.rs:592`), so a finished season's recorded band drifts with current form.

## Goals / Non-Goals

**Goals:**

- Stop the completion total (and every tier line) from drifting within a season.
- Guarantee the seasonal tier and band are **monotonic non-decreasing within a season** — no mid-season demotion.
- Make objective thresholds **stable for the season**.
- Make a finished season's recap standing **stable** (graded against that season's entry form, not today's).
- Add **no new persisted state** and **no core API change**.
- Preserve the determinism backbone and the read-only / offline invariants.

**Non-Goals:**

- Changing score weights, tier count, band names, or tier math.
- Changing the Today's-Progress comparison baseline — it *should* track current form and stays live.
- Introducing *mid-season* adaptivity. We considered a "partial adaptivity" middle ground and rejected it: anything that lets the bar respond to in-season output is the treadmill. The only coherent knob is *where the bar re-samples*, and the answer is season boundaries.
- Persisting a per-season target/baseline. We derive the pre-season window from existing history instead.

## Decisions

**D1 — Anchor the pacing baseline at the season start, not today.** Generalize the averaging helper to "the 30 most-recent active days with `day < anchor`." The Today tile passes `anchor = today` (identical to today's behavior); the season target passes `anchor = <season>-01`. For the current season the 371-day axis leaves ~340 pre-season calendar days to find 30 active ones, so the entry baseline is fully computable. Date strings are ISO `YYYY-MM-DD`, so the `day < anchor` compare is a plain lexicographic compare and the season anchor is `format!("{:04}-{:02}-01", year, month)`.

**D2 — Re-sample only at season boundaries.** Because the anchor is the season start, the window `[season start − 30 active days, season start)` does not move once the season is underway. The bar is therefore fixed for the season and re-baselines naturally at each boundary. This is the single product call the change forces, and it is binary (boundary-sampled vs. continuous); boundary-sampled is the reading that honors the rest of the product's never-demote promises.

**D3 — Objectives ride the same baseline.** Passing the entry baseline through `compute_season` freezes `objective_target` for the season for free — no separate objective change.

**D4 — Recap grades `prev` against its own entry baseline.** The recap callsite (`commands.rs:592`) builds a baseline anchored at `prev`'s first day instead of reusing the live one. The same generalized helper serves both the active season and the recap.

**D5 — Sparse or absent pre-season history falls through the existing clamps.** A first-ever or deep-backfilled season may find < 30 (or zero) pre-season active days; `target_total` then floors at `FLOOR_TOTAL` and objective targets hit their per-archetype floors. This is already-handled, bounded, and arguably correct (a brand-new developer gets the gentle floor).

**D6 — No core API change.** `compute_season` / `target_total` / `objective_target` / `season_recap` already take `baseline` by value. Only the *value* passed changes, plus the generalized averaging helper in `dashboard.rs`. The change is concentrated in `dashboard.rs` (helper) and `commands.rs` (two callsites).

## Risks / Trade-offs

- **A developer who genuinely levels up mid-season is graded against entry form**, making the back half of the month easier. Accepted: "beat your past self" is friendly, and it is exactly the property that removes the treadmill. Re-baselining at the next boundary restores the stretch.
- **Deep-backfilled seasons** whose pre-season window falls beyond the 371-day axis pace at the floor. Bounded and graceful: the **locker is monotonic** (`commands.rs:541`) so a thinner recompute can never revoke an unlock, and a recap is minted once (bookmarked by `last_recapped_season_index`), so its numbers are captured at mint time. The module's determinism guarantee was only ever scoped to the index-pure name/objectives/treatments, not the data-derived target — so anchoring the target to pre-season history is within contract and strictly more stable than the daily-sliding status quo.
- **Two baselines now exist** (live for the Today tile, entry for the season). Minor conceptual overhead, but they answer genuinely different questions: "how am I doing *right now*" vs. "what stretch did I walk into this season with."
