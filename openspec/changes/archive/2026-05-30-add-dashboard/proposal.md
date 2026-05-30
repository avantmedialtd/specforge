# Add a Dashboard

## Why

SpecForge's overview today is split across two surfaces that leave a gap between them. The tray/dock badge gives one number — the global active-change count — at a glance. The master-detail browser makes you click into one change at a time. There is nothing in between: no synthesized view of the *state of everything*.

The gap is most visible the moment you open the window. With workspaces registered but nothing selected, the center pane renders an `EmptyState` reading "Nothing selected — Pick a Proposal, Design, Tasks…". The home surface is a void. To learn anything beyond the badge count — how much work is in flight, how far along it is, which repos are busy, what changed recently — you have to drill into individual changes and assemble the picture in your head.

This change fills that void with a **Dashboard**: a global, all-workspaces overview rendered as the default home surface. It leads with the numbers the badge can't show (a cross-workspace task rollup, per-repo breakdown), and adds a *time* axis the app has never had on the OpenSpec side — a commits-per-day activity chart and change-throughput metrics mined from git history across every registered repo.

It is a deliberate, scoped evolution of the app's quiet stance. SpecForge stays ambient and read-only everywhere else — the tray, the notifications, the watcher are unchanged and gain no new chatter. The richness lives entirely behind the intentional act of opening the window and landing on the home surface; the Dashboard is read-only and computed on demand, not a new always-on signal.

## What Changes

- The center detail pane gains a new **Dashboard** render target. It is shown **by default at startup and whenever no artifact or commit is selected**, replacing the prior "Nothing selected" placeholder. It is reached deliberately via a **pinned "Dashboard" entry at the top of the tree pane** (mirroring the pinned Settings row at the bottom). Selecting an artifact or a commit replaces it; selecting the Dashboard entry returns to it.
- The Dashboard aggregates **globally across all registered workspaces**:
  - **Summary cards** — active change count, a global task rollup (`completed / total` and percent), the number of changes touching capability specs, and the registered-repo / worktree / flat-workspace counts.
  - **Per-repository breakdown** — one row per top-level entry (repo group or flat workspace) with its active and archived change counts.
  - **Activity over time** — a commits-per-day chart over a recent window, aggregated across every git-backed repo, built from `git log`.
  - **Change lifecycle metrics** — throughput (changes archived in the recent window) and average time-to-archive, derived from the commits that created and archived each change directory.
  - **Recent activity feed** — a time-ordered list of recently added / archived / modified changes, each selectable to jump to its detail.
- **Temporal data is mined from git history**, not persisted. The activity chart reuses the existing `commit_log`; lifecycle metrics come from a new single-pass `git log --diff-filter=A --name-status -- openspec/changes/` helper that recovers each change's creation and archive dates in one query per repo. **Task burn-down is explicitly deferred** (it would require diffing `tasks.md` across commits).
- The Dashboard **refreshes within the watcher's debounce window** on the existing cache and graph-changed event streams, while it is the active surface — no new watcher, no new event type.
- **Graceful degradation:** flat (non-git) workspaces and repos where `git` is unavailable contribute their counts but sit out the activity/lifecycle sections. The Dashboard never errors.
- New IPC command `get_dashboard`; new headless aggregator in `openspec-core`; new `DashboardView` component and a `{ kind: "dashboard" }` member of the `TreeSelection` union and the detail pane's render target.

## Capabilities

### New Capabilities

- `dashboard`: a global, read-only overview surface rendered as the default home target of the center pane — cross-workspace summary metrics, per-repository breakdown, a git-mined commits-per-day activity chart, change-lifecycle throughput and time-to-archive metrics, a recent-activity feed, reactive refresh on existing events, and graceful degradation when git is absent or a workspace is non-git.

### Modified Capabilities

- `spec-browser`: the *Master-Detail Layout* requirement changes so the center (detail) pane renders the Dashboard as an additional target — shown by default at startup and whenever no artifact or commit is selected — in place of the prior empty placeholder, reached via a pinned Dashboard entry at the top of the tree pane.

## Impact

- New module `crates/openspec-core/src/dashboard.rs` — the pure aggregator that turns registered workspaces, the cached `WorkspaceView`s, and git-mined activity/lifecycle data into a `DashboardData` payload. All IPC-facing types `#[serde(rename_all = "camelCase")]`, unit-tested from `cargo test` with no GUI, per the "watchers/parsers/git logic belong in the core" rule.
- `crates/openspec-core/src/git.rs` — new shell-outs: `change_lifecycle` (single-pass `git log --reverse --diff-filter=A --name-status -- openspec/changes/`, recovering per-change creation + archive commit dates) and bounded commit-activity extraction (`git log --all --since=… --pretty=%aI`). Same degrade-to-empty discipline as the existing functions.
- `crates/specforge/src/commands.rs` — a `#[tauri::command] get_dashboard` handler that calls the core aggregator over the current registry + cache snapshot.
- `src/types.ts` — hand-mirrored `DashboardData` and its nested types (camelCase parity with the serde structs); the `TreeSelection` union and the detail pane's `RenderTarget` each gain a `{ kind: "dashboard" }` variant.
- `src/api.ts` — an `invokeLogged` wrapper for `get_dashboard`.
- `src/hooks/` — a `useDashboard` hook that fetches `get_dashboard` and refetches on `cache-updated` / `change-added` / `change-archived` / `graph-changed`, mirroring `useWorkspaces` / `useCommitGraph` event wiring.
- `src/App.tsx` — render the `DashboardView` as the default center-pane target when nothing is selected; a `handleSelect` case for the `dashboard` selection; the pinned Dashboard entry at the top of the tree.
- `src/components/WorkspaceTree.tsx` — the pinned Dashboard entry row (mirroring the Settings footer row), at the top of the tree.
- New component `src/components/DashboardView.tsx`.
- **Deliberate scope boundaries** (so nobody "fixes" these later as oversights):
  - **Read-only and pull-based.** The Dashboard exposes no mutating action and computes on demand when it is the active surface; it adds no new ambient signal — the tray badge, dock badge, and notification behaviour are untouched. The "rich" surface lives behind opening the window, not in the always-on layer.
  - **Global only.** This change ships a single global overview across all workspaces. A drill-into-one-repo dashboard is a future option, not part of this change.
  - **Git-mined temporal data, no persisted event log.** Activity and lifecycle metrics come from `git log`. An event-log / time-series store was considered and rejected (empty until it warms up); git gives real history immediately.
  - **Task burn-down deferred.** Per-commit `tasks.md` diffing to chart task completion over time is out of scope; the rollup is a current-state snapshot.
  - **No "needs attention" triage this round.** Stale/diverged/artifact-gap surfacing — though the signals exist — is intentionally excluded from this change to keep scope tight.
  - **Thin git, no `git2`/`gix`.** All git access shells out to the system `git` binary, matching `git.rs` and keeping the Windows cross-compile pipeline free of a libgit2 C dependency.
