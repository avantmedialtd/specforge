## 1. Core: git-mined temporal data (`git.rs`)

- [x] 1.1 Add `change_lifecycle(repo_id)` — single pass `git --git-dir <common> log --all --reverse --no-renames --diff-filter=A --name-status --pretty=format:'<sep>%at'` scoped to `-- openspec/changes/`; parse added paths into per-change records: earliest add under `openspec/changes/<id>/…` → creation date, earliest add under `openspec/changes/archive/<id>/…` → archive date. Return a `Vec<ChangeLifecycle { change_name, created_at: Option<i64>, archived_at: Option<i64> }>` (epoch seconds via `%at`). `--no-renames` so an archive move surfaces as an Add under the archive path. Degrade to empty vec on git error
- [x] 1.2 Add `commit_activity(repo_id, since)` — `git --git-dir <common> log --all --since=<window> --pretty=format:%aI`; return the commit author-dates (ISO strings) in the window. Bounded read (no full-history scan). Degrade to empty on error
- [x] 1.3 Unit tests (mirroring the existing `git.rs` `tempfile` + `git` harness): a change created then archived in separate commits yields both dates; a change created but not archived yields a creation date and `None` archive date; `commit_activity` honours the `--since` bound (old ancestor + recent descendant, since `--since` prunes traversal at old commits); both degrade to empty outside a repo / with no `git`

## 2. Core: dashboard aggregator (`dashboard.rs`)

- [x] 2.1 Define the IPC-facing types, all `#[serde(rename_all = "camelCase")]`: `DashboardData { summary, repos, activity, activityWindowDays, lifecycle, recent }` plus `SummaryMetrics`, `RepoBreakdown`, `ActivityBucket`, `LifecycleMetrics`, `RecentEntry`
- [x] 2.2 Implement `compute_dashboard(views, now, window_days, recent_limit, activity_for, lifecycle_for)` — pure given the injected git closures (mirrors `compute_views`'s `default_branch_for`). `summary_metrics` folds the cached `WorkspaceView`s: active count, summed primary-instance `completedTasks`/`totalTasks`, specs-touching count, repo/worktree/flat counts; the percentage is guarded against a zero total
- [x] 2.3 Build the per-repo breakdown from each top-level `WorkspaceView`, using the tree's display name; active vs archived counts from `RepoView.active`/`.archived` (flat workspaces report 0 archived — the view carries no archived section)
- [x] 2.4 Aggregate the activity chart: call `commit_activity` per git-backed repo, bucket dates by the `%aI` day-prefix, sum across repos. (Buckets are days-with-commits; the frontend builds the viewer-local day axis and zero-fills — matching the commit-graph rail's local-day grouping.)
- [x] 2.5 Compute lifecycle metrics: throughput = changes whose archive date falls in the window; average time-to-archive = mean of `archived_at − created_at` over changes with both dates recoverable; `None` when none are recoverable
- [x] 2.6 Build the recent-activity feed: active changes across all workspaces, ordered by `modifiedAt` descending, capped, each carrying `worktreePath` + `changeId` for navigation
- [x] 2.7 Unit tests for the aggregator over hand-built inputs: summary sums + zero-task percentage guard; per-repo active/archived split; day-bucketing across repos; throughput windowing + average-time-to-archive with mixed recoverable/unrecoverable lifecycles; recent-feed ordering + cap; closures invoked only for Repo views (flats degrade to counts-only)

## 3. Shell: command

- [x] 3.1 Add `#[tauri::command] get_dashboard()` in `commands.rs` — snapshot the views, join presentation display-names (mirroring `get_workspace_views`), call `dashboard::compute_dashboard` off the async runtime with git closures, return the `DashboardData` payload
- [x] 3.2 Register the command in the shell's invoke handler

## 4. Frontend: types + API

- [x] 4.1 Mirror the new core types in `src/types.ts` by hand (`DashboardData` + nested `SummaryMetrics` / `RepoBreakdown` / `ActivityBucket` / `LifecycleMetrics` / `RecentEntry`) — camelCase parity with the serde structs
- [x] 4.2 Extend the detail pane's `RenderTarget` with a `{ kind: "dashboard" }` variant. (The `TreeSelection` union was intentionally NOT extended: the Dashboard entry is an App-level sidebar button mirroring the Settings entry, not a tree node — so it needs no `TreeSelection` member.)
- [x] 4.3 Add an `invokeLogged` wrapper in `src/api.ts` for `get_dashboard`

## 5. Frontend: hook

- [x] 5.1 Add a `useDashboard()` hook that fetches `get_dashboard` and refetches on `cache-updated`, `change-added`, `change-archived`, and `graph-changed` events — mounted only while the Dashboard is the active surface, so "refresh while shown" falls out of the component lifecycle (mirrors `useWorkspaces` / `useCommitGraph` wiring)

## 6. Frontend: home-surface integration

- [x] 6.1 In `App.tsx`, initialise `centerTarget` to `{ kind: "dashboard" }` and render `DashboardView` for that target (default at startup / when nothing is selected), replacing the prior empty placeholder
- [x] 6.2 Render a pinned "Dashboard" entry at the top of the sidebar — implemented in `App.tsx` as a `sidebar-header-button` mirroring the pinned Settings footer button (rather than inside `WorkspaceTree`, matching how the Settings entry is wired). Emits `selectDashboard` on click and renders an active treatment while the Dashboard is the current target
- [x] 6.3 Selecting an artifact (tree) or a commit (rail) replaces the Dashboard; selecting the Dashboard entry returns to it — last-selection-wins, consistent with the existing render-target model

## 7. Frontend: dashboard rendering (`DashboardView.tsx`)

- [x] 7.1 Summary cards: active-change count, task rollup (`completed / total` + percent + `--ok` meter), specs-touching count, repo / worktree / flat-workspace counts
- [x] 7.2 Per-repository breakdown: one row per top-level entry with active/archived counts + a relative-active bar, labelled with the tree's display name
- [x] 7.3 Activity chart: a commits-per-day bar strip over the window from `activity` buckets, with a viewer-local day axis and zero-fill
- [x] 7.4 Lifecycle metrics: throughput + average time-to-archive (shows `—` when no average is computable)
- [x] 7.5 Recent-activity feed: most-recent-first entries, each selectable to navigate to its change via the existing render-target
- [x] 7.6 Empty/degenerate states: zero registered workspaces (onboarding hint); all-flat / no-git (activity chart shows "No commits…", lifecycle shows `—`) — never errors

## 8. Spec sync (applied at archive time via `openspec archive`)

- [ ] 8.1 Apply the `dashboard` delta (new capability) from `openspec/changes/add-dashboard/specs/dashboard/spec.md`
- [ ] 8.2 Apply the `spec-browser` delta from `openspec/changes/add-dashboard/specs/spec-browser/spec.md` (modify *Master-Detail Layout* for the Dashboard render target + default landing)

## 9. Manual verification

- [ ] 9.1 Run `bun tauri dev` with several workspaces registered; confirm the Dashboard renders on launch in place of the "Nothing selected" placeholder, and that the pinned Dashboard entry returns to it after viewing an artifact or commit
- [ ] 9.2 Confirm the summary cards match reality: active count equals the tray badge, the task rollup sums across changes, the specs-touching and repo/worktree counts are correct
- [ ] 9.3 Confirm the per-repo breakdown lists every top-level entry with correct active/archived counts and the tree's display names
- [ ] 9.4 Confirm the activity chart shows commits-per-day across repos over the window; make a commit and confirm the chart updates within the debounce window
- [ ] 9.5 Archive a change on disk; confirm throughput and the active/archived counts update, and that the average time-to-archive is sensible
- [ ] 9.6 Register a non-git (flat) workspace alongside git repos; confirm it appears in counts and breakdown but contributes nothing to the activity chart, and nothing errors
- [ ] 9.7 Rename `git` off PATH (or register only a non-repo); confirm the activity and lifecycle sections degrade to empty/unavailable and the rest of the Dashboard still renders

## 10. Build check

- [x] 10.1 Run `bun run build` and confirm `tsc --noEmit` + Vite build succeed under `noUnusedLocals` / `noUnusedParameters` (the new `RenderTarget` variant, the hook, and `DashboardView` type-check)
- [x] 10.2 Run `cargo test` and confirm the new `dashboard.rs` and `git.rs` tests pass alongside the existing suite
