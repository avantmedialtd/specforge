# Tasks

## 1. Redefine the second hero tile as "in flight"

- [x] 1.1 In `src/components/DashboardView.tsx`, change `TodayHaul` so its second tile renders the live active-change count instead of `today.changesCreated`. Pass the active count into `TodayHaul` (from `summary.activeChanges`, already available in `DashboardView`), label it `in flight`, and keep the glyph and the second slot position (order stays 🏆 shipped · ✚ in flight · ⎇ commits · ✔ tasks done).
- [x] 1.2 Drop the `DeltaBadge` / average-comparison from the in-flight tile only. Leave `shipped`, `commits`, and `tasks done` with their average comparison intact. (Either branch `HaulTile` on whether an average is supplied, or render the in-flight tile without the badge.)
- [x] 1.3 Re-scope the `nothingYet` zero-state in `TodayHaul` to the three today-flow counts: show the "fresh day" nudge when `changesArchived`, `commitsLanded`, and `tasksCompleted` are all `0`, independent of the in-flight count.

## 2. Remove the redundant Overview "Active changes" card

- [x] 2.1 In `src/components/DashboardView.tsx`, remove the `dashboard-card` for `summary.activeChanges` ("Active changes") from the Overview analytics `dashboard-cards` section. Leave the Tasks, "Changes touch specs", and repos·worktrees cards and the footnote summary line (`{summary.activeChanges} active · {totalArchived} archived`) unchanged.

## 3. Retire the now-unused TodayProgress flow fields

- [x] 3.1 In `crates/openspec-core/src/dashboard.rs`, remove `changes_created` and `changes_created_avg_centi` from the `TodayProgress` struct, and remove their derivation in `compute_progress` (the `created_by_day` lookup for `today` and its trailing-average call). Keep `created_by_day` itself and `HeatmapCell.created` — the heatmap drill-down still needs per-day created counts.
- [x] 3.2 No assertions on `changes_created` / `changes_created_avg_centi` existed in `dashboard.rs` tests (the only `changes_created` references are on the unrelated `AchievementTotals` struct), so no test edits were needed.
- [x] 3.3 In `src/types.ts`, remove the mirrored `changesCreated` and `changesCreatedAvgCenti` fields from the `TodayProgress` type so the TypeScript mirror stays matched to the Rust struct.

## 4. Verify

- [x] 4.1 Run `cargo test -p openspec-core` and confirm the dashboard/activity-log tests pass.
- [x] 4.2 Run `bun run build` (tsc `--noEmit` + bundle) and confirm no type or build regressions (no unused-field references to the removed `TodayProgress` fields).
- [x] 4.3 Launch `bun tauri dev`, open the Dashboard, and confirm: the second tile reads **in flight** with the current active-change count and no "vs avg" badge; it reads `0` when no active changes remain; the Overview no longer shows an "Active changes" card; the footnote still shows `N active · M archived`.
- [x] 4.4 Click a populated heatmap day cell and confirm the drill-down still lists per-day `✚ N started` (created that day) in the shipped → started → commits → tasks order.
- [x] 4.5 Run `openspec validate replace-started-tile-with-in-flight --strict` and confirm the change validates.
