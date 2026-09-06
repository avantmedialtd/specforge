# Tasks

## 1. Carry the active count onto the garden payload

- [ ] 1.1 Add `active_count: usize` to `WorkspaceGarden` in `crates/openspec-core/src/garden.rs`, defaulting to `0` in the dormant constructors that build a plant without a view.
- [ ] 1.2 Fill it in `crates/openspec-app/src/service.rs` beside `plant.label`: `r.active.len()` for `WorkspaceView::Repo`, `changes.len()` for `WorkspaceView::Flat`.
- [ ] 1.3 Mirror the field in `src/types.ts`'s `WorkspaceGarden` as `activeCount: number`.
- [ ] 1.4 Test: a repository view with N active changes produces a plant whose `active_count` is N, and a flat workspace's plant carries its own change count rather than zero. This field is inside the mutation gate — assert the value, not merely its presence.

## 2. Order the garden's plots

- [ ] 2.1 Sort the plants in `crates/openspec-app/src/service.rs` before returning them: today's commit count descending, then label ascending.
- [ ] 2.2 Test the leading key: an entry with more commits today is returned before one with fewer, regardless of registration order.
- [ ] 2.3 Test the tiebreak adversarially: two entries with **equal** commit counts and labels that are out of order in the input come back in ascending label order, and a second call with the input permuted returns the same order. A test that only sorts already-sorted input leaves the comparator's mutants alive.
- [ ] 2.4 Test that the active-change count does not participate: two entries with equal commit counts and different active counts still order by label.

## 3. Extend the plot caption

- [ ] 3.1 In `src/components/CommitGarden.tsx`, add the active-change count to `garden-plot-count`, after the existing commits and conditional authors segments.
- [ ] 3.2 Keep the authors segment conditional on `authors > 1`, as today.
- [ ] 3.3 Confirm the caption reads `<label> · N commits · M authors · K active` with the authors segment omitted on a single-author day.

## 4. Remove the lifecycle metrics (not the mining)

- [ ] 4.1 Delete `lifecycle_metrics` and the `LifecycleMetrics` type from `crates/openspec-core/src/dashboard.rs`, together with their tests.
- [ ] 4.2 Remove the `lifecycle` and `lifecycle_window_days` fields from `DashboardData`, and drop `assemble`'s now-unused `window_days` parameter.
- [ ] 4.3 Delete `DASHBOARD_LIFECYCLE_WINDOW_DAYS` from `crates/openspec-app/src/service.rs` and its argument at the `assemble` call site — it has no other consumer.
- [ ] 4.4 Remove `LifecycleMetrics`, `lifecycleWindowDays` and `lifecycle` from `src/types.ts`.
- [ ] 4.5 Verify the mining path is untouched: `lifecycle_for` is still called once per repository in `assemble` and its result still reaches `repo_ships`. The existing per-repository invalidation, concurrency-collapsing and retry tests must still pass unmodified — if any needed editing, the mining was changed and that is out of scope.

## 5. Remove the analytics band from the Dashboard

- [ ] 5.1 Delete the `.dashboard-analytics` block from `src/components/DashboardView.tsx`: the rule, the `Overview` divider, the lifecycle span and the entire `Per repository` panel.
- [ ] 5.2 Delete `formatDuration` — the average-time-to-archive figure was its only caller.
- [ ] 5.3 Delete the `breakdown`, `remainder` and `maxShownActive` bindings; keep `totalArchived`'s reduction over `repos`, which the footnote still needs.
- [ ] 5.4 Delete `src/components/repoBreakdown.ts` and `src/components/repoBreakdown.test.ts`, and their import in `DashboardView.tsx`.
- [ ] 5.5 Remove the orphaned rules from `src/App.css`: `.dashboard-analytics`, `.dashboard-analytics-rule`, `.dashboard-analytics-divider`, `.dashboard-analytics .dashboard-panel`, `.dashboard-lifecycle` and its two descendant rules, and the nine `.dashboard-breakdown*` rules. Keep `.dashboard-panel` and `.dashboard-panel-title` — today's ships uses both.
- [ ] 5.6 Drop the sort from `repo_breakdowns` in `crates/openspec-core/src/dashboard.rs`, with its ordering tests; keep the vector, which the footnote's total reduces over. Keep the tests asserting the vector's membership and counts.

## 6. Verify

- [ ] 6.1 `bun run build` — type-check and rebuild the bundle. Required before any visual check: the debug `specforge-web` build serves `dist/` from disk, so a stale bundle renders the pre-change Dashboard.
- [ ] 6.2 `cargo fmt --check` and workspace clippy with `-D warnings`.
- [ ] 6.3 `cargo test` — the workspace suite, including the untouched lifecycle-mining tests.
- [ ] 6.4 `bun test` — the frontend suite, now without `repoBreakdown.test.ts`.
- [ ] 6.5 Mutation-test the diff: `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. The new comparator and the new field are the survivors to watch for.
- [ ] 6.6 Visual check in the browser loop (`specforge-serve` + `bun run dev`): the band is gone, the garden is the last section, captions carry the active count, and plots are ordered by today's commits.
- [ ] 6.7 Check the quiet-day path: with no commits today the garden omits itself and the Dashboard ends at the heatmap followed by the footnote, with no empty band left behind.

## 7. Sync the specs

- [ ] 7.1 Run `openspec sync` (or `/opsx:sync`) to apply both delta specs.
- [ ] 7.2 Edit the `dashboard` capability's `## Purpose` paragraph, which enumerates "a per-repository breakdown, change-lifecycle throughput and time-to-archive" — both clauses must go.
- [ ] 7.3 Grep the synced specs for `analytics band`, `Per-Repository Breakdown`, `Change Lifecycle Metrics` and `breakdown` to confirm no requirement still names a removed one.
