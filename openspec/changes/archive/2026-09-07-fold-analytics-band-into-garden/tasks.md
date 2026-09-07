# Tasks

## 1. Carry the active count onto the garden payload

- [x] 1.1 Add `active_count: usize` to `WorkspaceGarden` in `crates/openspec-core/src/garden.rs`, defaulting to `0` in the dormant constructors that build a plant without a view.
- [x] 1.2 Fill it in `crates/openspec-app/src/service.rs` beside `plant.label`: `r.active.len()` for `WorkspaceView::Repo`, `changes.len()` for `WorkspaceView::Flat`.
- [x] 1.3 Mirror the field in `src/types.ts`'s `WorkspaceGarden` as `activeCount: number`.
- [x] 1.4 Test: a repository view with N active changes produces a plant whose `active_count` is N, and a flat workspace's plant carries its own change count rather than zero. This field is inside the mutation gate — assert the value, not merely its presence.

## 2. Order the garden's plots

- [x] 2.1 Sort the plants in `crates/openspec-app/src/service.rs` before returning them: today's commit count descending, then label ascending.
- [x] 2.2 Test the leading key: an entry with more commits today is returned before one with fewer, regardless of registration order.
- [x] 2.3 Test the tiebreak adversarially: two entries with **equal** commit counts and labels that are out of order in the input come back in ascending label order, and a second call with the input permuted returns the same order. A test that only sorts already-sorted input leaves the comparator's mutants alive.
- [x] 2.4 Test that the active-change count does not participate: two entries with equal commit counts and different active counts still order by label.

## 3. Extend the plot caption

- [x] 3.1 In `src/components/CommitGarden.tsx`, add the active-change count to `garden-plot-count`, after the existing commits and conditional authors segments.
- [x] 3.2 Keep the authors segment conditional on `authors > 1`, as today.
- [x] 3.3 Confirm the caption reads `<label> · N commits · M authors · K active` with the authors segment omitted on a single-author day.

## 4. Remove the lifecycle metrics (not the mining)

- [x] 4.1 Delete `lifecycle_metrics` and the `LifecycleMetrics` type from `crates/openspec-core/src/dashboard.rs`, together with their tests.
- [x] 4.2 Remove the `lifecycle` and `lifecycle_window_days` fields from `DashboardData`, and drop `assemble`'s now-unused `window_days` parameter.
- [x] 4.3 Delete `DASHBOARD_LIFECYCLE_WINDOW_DAYS` from `crates/openspec-app/src/service.rs` and its argument at the `assemble` call site — it has no other consumer.
- [x] 4.4 Remove `LifecycleMetrics`, `lifecycleWindowDays` and `lifecycle` from `src/types.ts`.
- [x] 4.5 Verify the mining path is untouched: `lifecycle_for` is still called once per repository in `assemble` and its result still reaches `repo_ships`. The existing per-repository invalidation, concurrency-collapsing and retry tests must still pass unmodified — if any needed editing, the mining was changed and that is out of scope.

## 5. Remove the analytics band from the Dashboard

- [x] 5.1 Delete the `.dashboard-analytics` block from `src/components/DashboardView.tsx`: the rule, the `Overview` divider, the lifecycle span and the entire `Per repository` panel.
- [x] 5.2 Delete `formatDuration` — the average-time-to-archive figure was its only caller.
- [x] 5.3 Delete the `breakdown`, `remainder` and `maxShownActive` bindings; keep `totalArchived`'s reduction over `repos`, which the footnote still needs.
- [x] 5.4 Delete `src/components/repoBreakdown.ts` and `src/components/repoBreakdown.test.ts`, and their import in `DashboardView.tsx`.
- [x] 5.5 Remove the orphaned rules from `src/App.css`: `.dashboard-analytics`, `.dashboard-analytics-rule`, `.dashboard-analytics-divider`, `.dashboard-analytics .dashboard-panel`, `.dashboard-lifecycle` and its two descendant rules, and the nine `.dashboard-breakdown*` rules. Keep `.dashboard-panel` and `.dashboard-panel-title` — today's ships uses both.
- [x] 5.6 Drop the sort from `repo_breakdowns` in `crates/openspec-core/src/dashboard.rs`, with its ordering tests; keep the vector, which the footnote's total reduces over. Keep the tests asserting the vector's membership and counts.

## 6. Verify

- [x] 6.1 `bun run build` — type-check and rebuild the bundle. Required before any visual check: the debug `specforge-web` build serves `dist/` from disk, so a stale bundle renders the pre-change Dashboard.
- [x] 6.2 `cargo fmt --check` and workspace clippy with `-D warnings`.
- [x] 6.3 `cargo test` — the workspace suite, including the untouched lifecycle-mining tests.
- [x] 6.4 `bun test` — the frontend suite, now without `repoBreakdown.test.ts`.
- [x] 6.5 Mutation-test the diff: `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. The new comparator and the new field are the survivors to watch for. **Green: 7 mutants — 2 caught, 5 unviable, 0 survived, on an `ok` unmutated baseline.** Diffed against the working tree (not `HEAD`), since the implementation was still uncommitted when the gate ran.
- [x] 6.6 Visual check in the browser loop (`specforge-serve` + `bun run dev`): the band is gone, the garden is the last section, captions carry the active count, and plots are ordered by today's commits. **Verified live against a debug `specforge-serve`** (read-only, real config, nothing registered or renamed). Served bundle hash matched the worktree's `dist/`, so no stale-bundle false pass. `.dashboard-analytics`, `.dashboard-analytics-rule`, `.dashboard-breakdown-row` and `.dashboard-lifecycle` all count 0; the rendered text contains no "Overview", "Per repository" or "time-to-archive". Section order is hero → haul → ships → heatmap → garden → footnote span, so the garden is the last section. Captions read e.g. `MushRoom · 14 commits · 1 active`. Order came back `14, 2, 2, 1, 1` with the label tiebreak firing twice on real data — `SpecForge` before `TME ODE`, `Istvan.xyz` before `Meter Burn`.
- [x] 6.7 Check the quiet-day path: with no commits today the garden omits itself and the Dashboard ends at the heatmap followed by the footnote, with no empty band left behind. **Filter verified live:** the payload carried 13 entries and the section rendered 5, omitting 8 dormant/quiet ones, and every entry carried an `activeCount`. The zero-case early return (`if (active.length === 0) return null`, `CommitGarden.tsx:142`) is untouched by this change, so the whole-section omission behaves as before. Not exercised at literally zero — that would have meant mutating the real registry, which this read-only check deliberately avoided.

## 7. Sync the specs

- [x] 7.1 Run `openspec sync` (or `/opsx:sync`) to apply both delta specs.
- [x] 7.2 Edit the `dashboard` capability's `## Purpose` paragraph, which enumerates "a per-repository breakdown, change-lifecycle throughput and time-to-archive" — both clauses must go.
- [x] 7.3 Grep the synced specs **case-insensitively** for `analytics band`, `per-repository breakdown`, `lifecycle metric`, `time-to-archive`, `throughput` and `breakdown`, to confirm no requirement still names a removed one. Case matters: an earlier version of this sweep searched only the capitalised requirement titles and would have reported clean while *Reactive Dashboard Updates* still said "and lifecycle metrics" in lower case.

  The widened sweep paid for itself immediately, catching a stale reference in a **third** capability that neither the change nor the code review had identified: `workspace-registry`'s *Disabling Preserves Row Identity* asserted that a disabled row keeps its label "including the Dashboard's per-repository breakdown and today's ships". Re-anchored to the commit garden. Every remaining hit across the 35 specs is a deliberate prohibition ("no analytics band is rendered…", "SHALL NOT present aggregate lifecycle statistics"), verified by re-running the sweep with those excluded.

## 8. Post-review fixes

Findings from the `/code-review max` pass. Grouped here rather than folded into the sections above, so the review's effect on the change is legible.

- [x] 8.1 Extract the plot comparator into `openspec-core`: `plot_order` + `sort_plots` in `garden.rs`, called from `service.rs`. `cargo mutants` replaces whole function bodies, so the previous inline `sort_by` closure generated **zero** mutants and the gate was structurally blind to it.

  Measured outcome, stated precisely because the obvious claim would be too strong: the gate went from 7 mutants / 2 caught to 9 / 3 caught, 0 survived. Of the two new mutants, `replace sort_plots with ()` is viable and **caught** — so "the sort is applied at all" is now gated. `replace plot_order -> Ordering with Default::default()` is **unviable** on this toolchain, because `std::cmp::Ordering` implements no `Default`, so it never compiles and never runs. The comparator's *key choice and direction* therefore remain covered by the pure unit tests in `garden.rs`, not by the gate. That is the reason task 8.3's anti-correlated fixture and the totality test are load-bearing rather than belt-and-braces.
- [x] 8.2 Add a stable `entry_key` (repo id / flat workspace URI) to `WorkspaceGarden` and make it the comparator's third key. Display labels are not unique — two worktrees of unrelated repos share a basename — so without it two entries sharing a label fell back to registry order, which *Deterministic Plot Order* forbids by name.
- [x] 8.3 Re-home the ordering tests as pure unit tests in `openspec-core::garden`, and **anti-correlate** the active-count fixture. The previous fixture was `zulu=3, alpha=1, mike=2` asserted as `alpha(1), mike(2), zulu(3)` — labels and active counts both ascending, so an ascending active-count tiebreak passed it. Added a totality test asserting no two distinct entries compare `Equal`.
- [x] 8.4 Strengthen the surviving integration test with `!dormant` and commit-count assertions. It previously asserted labels only, so it would have passed unchanged if every plant came back dormant and empty — they would all tie at zero and the label tiebreak would reproduce the expected list.
- [x] 8.5 Fix the vacuous assertion in `unchanged_repository_is_mined_at_most_once_across_two_fetches`: its fixture creates no archive directory, so `todays_ships` was `[]` on both fetches and the equivalence assertion was `[] == []`. The fixture now archives a change dated today, and the test asserts the feed is populated *before* comparing.
- [x] 8.6 Give the TUI's garden caption the active-change count (`ui.rs`). It renders captions from the same payload and already held `active_count`, so it violated the new *Plot Caption* requirement on arrival, against `terminal-ui`'s frontend-parity clause.
- [x] 8.7 Suppress the active count at zero, matching the authors segment's existing rule, and guard `activeCount` with `?? 0` — the documented dev loop runs a newer frontend against a separately-built server, which would have rendered "· undefined active" with no error.
- [x] 8.8 Give `.garden-plot-label` a width floor and let `.garden-plot-count` shrink. The count was `flex-shrink: 0` against a label with no floor, so the unconditional new segment stole width from the only element naming the repository.
- [x] 8.9 Key the plot list on `entryKey` rather than the array index. The list is now reorderable on every refetch, so an index key remounted every plot below a rank change and reset any panned day-DAG gutter.
- [x] 8.10 Document `GARDEN_COMMIT_LIMIT`'s effect on the new leading sort key: the window is committer-dated and the filter is author-dated, so a large fetch can demote or hide an entry whose work happened today. Comment only — the fix is a separate change.
- [x] 8.11 Qualify the *No per-repository breakdown is rendered* scenario, which as written forbade the ranked per-repository list the sibling `commit-garden` delta mandates on the same Dashboard.
- [x] 8.12 Add *Reactive Dashboard Updates* to the delta — it still required the Dashboard to refresh "lifecycle metrics", and was not among the modified requirements.
- [x] 8.13 Restore the scenarios the two over-replacing MODIFIED blocks dropped. `Dashboard Includes Disabled Workspaces` went from 8 scenarios to 2, losing the parked-ship click path, the streak/heatmap clause, the every-workspace-disabled case and the cache-derived-fields invariant that licenses the garden's own `r.active.len()` read. `Personal Progress Frame` lost the alias-folding and cross-author-ranking scenarios.
- [x] 8.14 Update the published docs at `site/pages/docs/dashboard/+Page.tsx`, which still described the per-repository breakdown and the change-lifecycle figures. A master push touching `site/**` publishes live.
- [x] 8.15 Widen task 7.3's grep sweep to be case-insensitive and to cover `lifecycle metric` / `time-to-archive` / `throughput`.
