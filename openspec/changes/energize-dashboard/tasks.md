## 1. Core: activity-log event store

- [ ] 1.1 Add `activity_log.rs` to `openspec-core` with an `Achievement` event type (`type`, `timestamp`, `workspace`, `changeId`, `magnitude`) — all `#[serde(rename_all = "camelCase")]`
- [ ] 1.2 Implement append-only persistence to the app data directory (path derived like `settings.rs`; never under any workspace `openspec/`)
- [ ] 1.3 Implement `record(event)` and a bounded, day-bucketed `query(window)` plus cumulative totals for milestone evaluation
- [ ] 1.4 Unit-test append/read round-trip, bounded query, and local-day bucketing

## 2. Core: achievement detection from watcher re-parses

- [ ] 2.1 At cache replacement in `watcher.rs`, diff previous vs. new `Vec<ChangeData>` per workspace
- [ ] 2.2 Emit net-positive achievements: completed-task increase (magnitude = delta), artifact status advance, change created, change archived
- [ ] 2.3 Ensure decreases (unchecks, deleted task lines, removed changes) emit nothing
- [ ] 2.4 Unit-test the diff with synthetic before/after parses (increase, decrease, add, archive)

## 3. Core: git backfill

- [ ] 3.1 Backfill creation/archival/commit signals over a bounded window, reusing the commit-graph mining path and lifecycle date mining
- [ ] 3.2 Backfill task completions by diffing `tasks.md` checkbox state across commits (windowed; may run after the cheap signals)
- [ ] 3.3 Trigger backfill only when a git-backed workspace has no prior log history; flat workspaces contribute nothing
- [ ] 3.4 Unit-test backfill bounding and non-git degradation

## 4. Core: dashboard aggregation

- [ ] 4.1 Extend `DashboardData` with `today` (per-type counts + average comparison), `streak`, `heatmap[]`, and `milestones[]`
- [ ] 4.2 Compute today's counts, trailing-30-active-day averages, streak (any-achievement day), and bounded heatmap buckets from the activity log
- [ ] 4.3 Derive milestone thresholds (first ship; 10/50/100 tasks; 5/10/25 ships; 3/7/30-day streaks) from cumulative totals; flag backfilled-vs-live for celebration suppression
- [ ] 4.4 Unit-test aggregation against a synthetic activity log

## 5. Shell: command + wiring

- [ ] 5.1 Wire the activity log into the core handle in `lib.rs` (initialize, run backfill on startup before first `get_dashboard`)
- [ ] 5.2 Extend `get_dashboard` payload; confirm it refreshes on existing cache/graph events
- [ ] 5.3 Mirror new Rust types in `src/types.ts` (camelCase)
- [ ] 5.4 Extend `getDashboard()` consumption in `src/api.ts` if the surface changed

## 6. Frontend: Today's Progress hero

- [ ] 6.1 Add the Today's Progress band (tasks / ships / commits / started) above existing content in `DashboardView.tsx`
- [ ] 6.2 Implement count-up animation with a `prefers-reduced-motion` guard (final value, no tween)
- [ ] 6.3 Render the per-type "▲/▼ vs average" comparison; render the warm zero state when nothing today

## 7. Frontend: streak + heatmap

- [ ] 7.1 Add the streak counter to the hero strip
- [ ] 7.2 Add the GitHub-style bounded heatmap with today's cell distinguished

## 8. Frontend: milestones panel

- [ ] 8.1 Add a "recent milestones" panel listing the most recently crossed badges

## 9. Frontend: live celebration

- [ ] 9.1 Fire confetti on the existing `change-archived` event only while the Dashboard is the active surface
- [ ] 9.2 Add a quieter glow on a task-completed signal; suppress all motion under `prefers-reduced-motion`; never block interaction
- [ ] 9.3 Ensure backfilled/pre-existing milestones never trigger celebration

## 10. Frontend: layout + styling

- [ ] 10.1 Demote (do not remove) the summary cards, per-repo breakdown, and lifecycle averages below the progress band
- [ ] 10.2 Style the new sections in `App.css` using existing custom properties; verify responsive breakpoints

## 11. Verification

- [ ] 11.1 `cargo test` (workspace) green, including new core tests
- [ ] 11.2 `bun run build` (tsc + bundle) clean
- [ ] 11.3 Manual verification with `bun tauri dev`: today's counts update on edits, confetti on archive, streak/heatmap populate from backfill, reduced-motion respected, empty/non-git workspace degrades gracefully
