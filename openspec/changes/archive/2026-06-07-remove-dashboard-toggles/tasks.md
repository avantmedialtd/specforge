# Tasks

## 1. Frontend — remove the toggles

- [x] 1.1 Delete the `ScopeToggle` component and the `LensToggle` component from `DashboardView.tsx`
- [x] 1.2 Remove the `scope` and `lens` `useState` hooks; call `useDashboard()` with no args
- [x] 1.3 Remove the `dashboard-toggles` wrapper from the hero (and the now-unused `DashboardScope` / `DashboardLens` imports)
- [x] 1.4 Confirm the season home, season ladder, career rank, leaderboards, streak, and celebrations still render unchanged

## 2. IPC surface + types

- [x] 2.1 `src/hooks/useDashboard.ts` — drop the `scope` and `lens` arguments
- [x] 2.2 `src/api.ts` — `getDashboard()` loses both params; remove the `DashboardScope` / `DashboardLens` imports
- [x] 2.3 `src/types.ts` — remove the `DashboardScope` and `DashboardLens` types

## 3. Rust command

- [x] 3.1 `crates/specforge/src/commands.rs` — `get_dashboard` drops the `scope` and `lens` params
- [x] 3.2 Collapse `only_me` to always-true: delete the *Everyone* branches for `scoped_achievements`, `commit_days`, and the in-flight count (keep `scoped_in_flight`)
- [x] 3.3 Collapse `season_lens` to always-false: `data.progress = base_progress` directly; delete the season-lens branch
- [x] 3.4 `crates/openspec-core/src/dashboard.rs` — keep the me-vs-everyone unit test (it drives `compute_progress` directly); rename it for clarity now that no toggle reaches it

## 4. CSS

- [x] 4.1 `src/App.css` — remove the `.dashboard-toggles`, `.scope-toggle`, and `.scope-toggle-btn` rules

## 5. Verification

- [x] 5.1 `bun run build` (tsc `--noEmit` + bundle) passes — no unused-locals/params or type regressions from the dropped params
- [ ] 5.2 `cargo test` passes (workspace), including the renamed `compute_progress` test
- [ ] 5.3 Visual check in the running app: hero shows no segmented controls; gamified tiles show the developer's all-time activity; leaderboard and season home intact
- [ ] 5.4 Confirm the in-flight tile counts the developer's active changes (unchanged from the prior default)

## 6. Spec sync

- [ ] 6.1 On archive, sync the `dashboard` delta into `openspec/specs/dashboard/spec.md` (2 removed, 1 added, 2 modified)
