# Replace the "started" progress tile with a live "in flight" count

## Why

The Today's Progress band's second tile is labelled **started** and shows `changesCreated` — the number of changes whose creation was *recorded today*. That is a daily-flow achievement count, deliberately parallel to `shipped`, `commits`, and `tasks done` (dashboard spec, *Today's Progress Hero*).

But "Started" reads as a *state* — "changes I have on the go" — not as "changes I created today". The two collide whenever a change is created and shipped on the same day: it ticks both `started` and `shipped`. On a productive day you can clear every change to the archive yet still see **started 3**, because three of the day's ships were also *born* that day. With zero active changes remaining, "started 3" looks like a bug:

```
   FLOW reading (what the tile is)        STATE reading (what the user expects)
   ──────────────────────────────        ─────────────────────────────────────
   started = changeCreated events          started = changes not yet shipped
   stamped today                           → 0 when everything is archived
   → 3 created today, also shipped         (the intuitive reading)
   → still shows 3
```

Confirmed against the live activity log: on the reported day, three `changeCreated` events were dated today (`energize-dashboard`, `area51-home-cta-distinct-photo`, `reorder-dashboard-progress-tiles`) and all three were also among the day's ships — so `started` correctly, but confusingly, read 3 while no active change remained.

The fix is to make the tile mean what it looks like it means: the count of changes **currently in flight** — active (non-archived) changes across all workspaces, right now. When everything is shipped it reads `0`, matching intuition. The exact figure already exists as `summary.activeChanges` (the same count the tray badge and the footnote use), so no new aggregation is needed.

## What Changes

- **Redefine the second hero tile** from `started` (`changesCreated`, a today-flow count) to **in flight** — the live count of active (non-archived) changes across all workspaces, sourced from the existing `summary.activeChanges`. Glyph and slot position (second) are retained; the label changes to `in flight`.
- **Drop the "▲ vs avg" comparison badge on this tile.** A live level has no meaningful trailing daily average to compare against (the activity log stores events, not daily active-count snapshots). The other three tiles (`shipped`, `commits`, `tasks done`) keep their average comparison.
- **Scope the zero-state nudge to the three remaining today-flow counts.** The "fresh day" nudge now appears when `shipped`, `commits`, and `tasks done` are all zero for the day — independent of the in-flight count (having changes in flight is not something you did *today*).
- **Remove the now-redundant "Active changes" card** from the Overview analytics section. The active-change count is now the hero's in-flight tile; the footnote summary line (`N active · M archived`) is retained, so the Cross-Workspace Summary Metrics requirement stays satisfied.
- **Retire the unused `changesCreated` / `changesCreatedAvgCenti` fields** from the `TodayProgress` IPC type (and their computation), since no surface renders them after this change. The per-day "started" breakdown in the heatmap drill-down is unaffected — it reads the separate `HeatmapCell.created`, which stays.

The heatmap day drill-down continues to show per-day **started** (changes created on that specific past day) via `HeatmapCell.created` — a per-day historical fact that does not suffer the state/flow confusion and has no live-state equivalent.

## Impact

- **Affected specs:** `dashboard` (*Today's Progress Hero* — second count redefined from a today-flow created count to a live in-flight count, average-comparison scoped to the three flow counts, zero-state scoped to the flow counts; *Streak and Contribution Heatmap* — drill-down ordering decoupled from the band, since in-flight has no per-day equivalent).
- **Affected code:**
  - `src/components/DashboardView.tsx` — `TodayHaul` second tile reads the active-change count and drops its `DeltaBadge`; `nothingYet` keyed on the three flow counts; remove the Overview "Active changes" card.
  - `src/types.ts` + `crates/openspec-core/src/dashboard.rs` — remove `changesCreated` / `changesCreatedAvgCenti` from `TodayProgress` and their derivation in `compute_progress`; `HeatmapCell.created` and the `created_by_day` map are retained for the heatmap.
- **No change** to git mining, the activity-log event model, the archived/active aggregation, or any other tile.
