## 1. Core: activity-log event store

- [x] 1.1 Add `activity_log.rs` to `openspec-core` with an `Achievement` event type (`type`, `timestamp`, `workspace`, `changeId`, `magnitude`) — all `#[serde(rename_all = "camelCase")]`
- [x] 1.2 Implement append-only persistence to the app data directory (path derived like `settings.rs`; never under any workspace `openspec/`)
- [x] 1.3 Implement `record(event)` and a bounded, day-bucketed `query(window)` plus cumulative totals for milestone evaluation
- [x] 1.4 Unit-test append/read round-trip, bounded query, and local-day bucketing

## 2. Core: achievement detection from watcher re-parses

- [x] 2.1 At cache replacement in `watcher.rs`, diff previous vs. new `Vec<ChangeData>` per workspace
- [x] 2.2 Emit net-positive achievements: completed-task increase (magnitude = delta), artifact status advance, change created, change archived
- [x] 2.3 Ensure decreases (unchecks, deleted task lines, removed changes) emit nothing
- [x] 2.4 Unit-test the diff with synthetic before/after parses (increase, decrease, add, archive)

## 3. Core: git backfill

- [x] 3.1 Backfill creation/archival/commit signals over a bounded window, reusing the commit-graph mining path and lifecycle date mining
- [x] 3.2 Backfill task completions by diffing `tasks.md` checkbox state across commits (windowed; may run after the cheap signals)
- [x] 3.3 Trigger backfill only when a git-backed workspace has no prior log history; flat workspaces contribute nothing
- [x] 3.4 Unit-test backfill bounding and non-git degradation

## 4. Core: dashboard aggregation

- [x] 4.1 Extend `DashboardData` with `today` (per-type counts + average comparison), `streak`, `heatmap[]`, and `milestones[]`
- [x] 4.2 Compute today's counts, trailing-30-active-day averages, streak (any-achievement day), and bounded heatmap buckets from the activity log
- [x] 4.3 Derive milestone thresholds (first ship; 10/50/100 tasks; 5/10/25 ships; 3/7/30-day streaks) from cumulative totals; flag backfilled-vs-live for celebration suppression
- [x] 4.4 Unit-test aggregation against a synthetic activity log

## 5. Shell: command + wiring

- [x] 5.1 Wire the activity log into the core handle in `lib.rs` (initialize, run backfill on startup before first `get_dashboard`)
- [x] 5.2 Extend `get_dashboard` payload; confirm it refreshes on existing cache/graph events
- [x] 5.3 Mirror new Rust types in `src/types.ts` (camelCase)
- [x] 5.4 Extend `getDashboard()` consumption in `src/api.ts` if the surface changed (no signature change — activity log is backend state; payload now carries `progress`)

## 6. Frontend: Today's Progress hero

- [x] 6.1 Add the Today's Progress band (tasks / ships / commits / started) above existing content in `DashboardView.tsx`
- [x] 6.2 Implement count-up animation with a `prefers-reduced-motion` guard (final value, no tween)
- [x] 6.3 Render the per-type "▲/▼ vs average" comparison; render the warm zero state when nothing today

## 7. Frontend: streak + heatmap

- [x] 7.1 Add the streak counter to the hero strip
- [x] 7.2 Add the GitHub-style bounded heatmap with today's cell distinguished

## 8. Frontend: milestones panel

- [x] 8.1 Add a "recent milestones" panel listing the most recently crossed badges

## 9. Frontend: live celebration

- [x] 9.1 Fire confetti on the existing `change-archived` event only while the Dashboard is the active surface
- [x] 9.2 Add a quieter glow on a task-completed signal; suppress all motion under `prefers-reduced-motion`; never block interaction
- [x] 9.3 Ensure backfilled/pre-existing milestones never trigger celebration

## 10. Frontend: layout + styling

- [x] 10.1 Demote (do not remove) the summary cards, per-repo breakdown, and lifecycle averages below the progress band
- [x] 10.2 Style the new sections in `App.css` using existing custom properties; verify responsive breakpoints

## 11. Verification

- [x] 11.1 `cargo test` (workspace) green, including new core tests (0 failures across all suites)
- [x] 11.2 `bun run build` (tsc + bundle) clean (0 TS errors)
- [~] 11.3 `bun tauri dev`: app launches and runs clean — no "state not managed", no panic, process alive; `get_dashboard` resolves `activityLog` state (fixed by managing it before the backfill); activity.json seeded by the background git backfill (365 events: 241 archived, 105 created, 19 tasks — all `backfilled:true`, so first launch fires no celebration). Pending a final human visual pass of the rendered hero / heatmap / confetti / reduced-motion in the running window.
