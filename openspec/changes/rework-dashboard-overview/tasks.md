## 1. Frontend: retire the commits chart

This change is a deletion, so the usual core-first order is reversed for the
chart: removing the payload field before its reader would leave `bun run build`
red between groups. The consumer goes first, and group 2 then removes a field
nothing reads.

- [x] 1.1 Delete the `ActivityChart` component and the `buildAxis` helper from `src/components/DashboardView.tsx`, and remove the chart's `<section>` from the analytics band (`dashboard`: *Analytics Band Composition*)
- [x] 1.2 Stop destructuring `activity` and `activityWindowDays` in `DashboardView`, keeping `lifecycle` bound — its new placement lands in task 4.1
- [x] 1.3 Delete `.dashboard-chart`, `.dashboard-chart--empty`, `.dashboard-bar-col` and `.dashboard-bar` from `src/App.css`
- [x] 1.4 Run `bun run build` — strict `tsc` with `noUnusedLocals`/`noUnusedParameters` fails on any helper or import the deletion orphaned

## 2. Core: remove the commit-bucket payload

- [x] 2.1 In `crates/openspec-core/src/dashboard.rs`, delete `ActivityBucket`, `bucket_activity`, the `activity` field of `DashboardData`, the `activity_dates` accumulator in `compute_dashboard`, and `compute_dashboard`'s `activity_for` closure parameter — the closure exists only to feed the buckets and has no other consumer
- [x] 2.2 Rename `DashboardData::activity_window_days` to `lifecycle_window_days` and rewrite its doc comment: it now documents the throughput window, not a chart axis (`dashboard`: *Change Lifecycle Metrics*)
- [x] 2.3 Drop `ActivityBucket` from the `pub use dashboard::{…}` re-export in `crates/openspec-core/src/lib.rs`
- [x] 2.4 In `crates/openspec-app/src/service.rs`, delete `activity_dates_since`, the `activity_cutoff` binding and the `activity_by_repo` map, and drop the matching argument at the `compute_dashboard` call site — leave `commit_activity_with_authors` and `commit_activity_cache` untouched, since the heatmap and leaderboard still walk them
- [x] 2.5 Rename `DASHBOARD_ACTIVITY_WINDOW_DAYS` to `DASHBOARD_LIFECYCLE_WINDOW_DAYS` in `crates/openspec-app/src/service.rs` and in its re-export in `crates/openspec-app/src/lib.rs`; the value and its role as the throughput window are unchanged
- [x] 2.6 Update the in-module tests in `dashboard.rs` that assert `data.activity_window_days` and `data.activity.len()` and that pass an `activity_for` fixture closure to `compute_dashboard`
- [x] 2.7 Mirror the payload change in `src/types.ts`: delete the `ActivityBucket` interface and the `activity` field of the dashboard payload, and rename `activityWindowDays` to `lifecycleWindowDays`

## 3. Core: order the per-repository breakdown

- [x] 3.1 Sort the result of `repo_breakdowns` in `crates/openspec-core/src/dashboard.rs` by active count descending, then archived count descending, then label ascending (`dashboard`: *Per-Repository Breakdown*)
- [x] 3.2 Add a `repo_breakdowns` test whose fixture carries a deliberate tie in the active count and a further tie in the archived count, asserting the full resulting order rather than membership — a comparator missing its last key passes any assertion made on a fixture with no ties
- [x] 3.3 Add a test that a repository's ordering position is unchanged by its disabled state, alongside the existing disabled-row coverage in `crates/openspec-app/tests/workspace_management.rs` (`dashboard`: *Dashboard Includes Disabled Workspaces*)

## 4. Frontend: rebuild the Overview band

- [x] 4.1 Move the throughput and average-time-to-archive figures out of the removed chart card and onto the band's divider row in `src/components/DashboardView.tsx`, band label left and figures right (`dashboard`: *Analytics Band Composition*)
- [x] 4.2 Name the window in the figures themselves — "39 archived · 14 days · avg time-to-archive 1.1d" — reading the length from `lifecycleWindowDays`, since no neighbouring caption supplies it any more (`dashboard`: *Change Lifecycle Metrics*)
- [x] 4.3 Make the band a single full-width column and delete `.dashboard-grid` and its `@media (max-width: 720px)` override from `src/App.css` — the breakdown is its only remaining member
- [x] 4.4 Add the divider-row rules to `src/App.css`: the label and figures on one baseline, figures in `--font-mono` with `font-variant-numeric: tabular-nums`, wrapping below the label at narrow widths

## 5. Frontend: cap and re-render the breakdown

- [x] 5.1 Add a pure module (for example `src/components/repoBreakdown.ts`) that takes the ordered payload array and returns the entries to present plus the remainder counts, capping at $N = 5$ — keeping the logic out of the component so `bun test` can reach it, matching the pattern of `src/components/graphGeometry.ts`
- [x] 5.2 Render the remainder line from that module's output: the number of withheld entries, and how many of them have active changes when any do — never the registry-wide archived total, which the page footnote below already carries (`dashboard`: *Per-Repository Breakdown*)
- [x] 5.3 Render the two row shapes in `DashboardView`: an entry with active changes draws a proportional bar; an entry with none draws no track at all and is de-emphasised, showing only its label and archived count
- [x] 5.4 Replace `.dashboard-breakdown-track`'s `flex: 1` in `src/App.css` with a fixed maximum length so a bar encodes a count rather than the pane's width, and give `.dashboard-breakdown-counts` `font-variant-numeric: tabular-nums` so the columns align (`dashboard`: *Per-Repository Breakdown*)
- [x] 5.5 Add `src/components/repoBreakdown.test.ts` pinning the cap boundary at exactly $N$ and at $N+1$ entries, the remainder wording with and without withheld active work, and the no-remainder-line case for a registry smaller than the cap
- [x] 5.6 Verify the page footnote still reports the registry-wide archived total by summing the full payload array, not the capped slice (`dashboard`: *Per-Repository Breakdown*, scenario *Capping does not reduce the registry-wide totals*)

## 6. Frontend: promote today's ships

- [x] 6.1 Move the today's-ships `<section>` in `src/components/DashboardView.tsx` above `<Heatmap>`, leaving it below `<TodayHaul>` (`dashboard`: *Dashboard Section Order*)
- [x] 6.2 Leave the quiet-day note in place and unconditional, so the section holds its position on a day with no ships (`dashboard`: *Today's Ships Quiet State*)

## 7. Verification

- [x] 7.1 Run `cargo test` for the workspace — `bun install && bun run build` first if `dist/` is absent in this worktree, since `generate_context!` and `RustEmbed` need it at compile time
- [x] 7.2 Run `cargo fmt --check` and workspace `cargo clippy -- -D warnings`; both gate CI
- [x] 7.3 Run `bun test` and `bun run build`
- [x] 7.4 Run `cargo mutants --in-diff` against the merge-base with `origin/master`; the ordering comparator in `repo_breakdowns` is the mutant most likely to survive, and a survivor there means task 3.2's fixture has no tie in it
- [x] 7.5 Smoke the change in the running app via `bun run wt:dev`, walking the scenarios: today's ships sits above the heatmap; the band's divider carries the lifecycle figures and names the window; the breakdown is ordered by active changes and shows at most five entries with a remainder line; entries with no active changes show no bar; widening the window does not lengthen a bar
- [x] 7.6 Run `openspec validate rework-dashboard-overview --strict`
- [ ] 7.7 When syncing the deltas into `openspec/specs/dashboard/spec.md`, remove "a git-mined commits-per-day activity chart" from the capability's `## Purpose` paragraph — it survives the delta blocks, which only reach requirements
