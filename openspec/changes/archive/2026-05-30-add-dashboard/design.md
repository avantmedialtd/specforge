# Design — Add a Dashboard

## Decisions

### The Dashboard is the home surface, not a new pane

The Dashboard is a new render target for the existing center (detail) pane, shown by default at startup and whenever no artifact or commit is selected. It is **not** a fourth pane, a separate window, or a Settings-style toggle.

- **Why:** the app already has a void exactly where the Dashboard belongs — the center pane renders a "Nothing selected" `EmptyState` on launch. The Dashboard replaces that placeholder, so it fills an existing gap rather than adding a new surface. Structurally it slots into the same mechanism the commit-detail view already uses: the center pane renders whichever target was last selected, and the Dashboard is simply the default when neither the tree nor the rail has selected anything.
- **Entry affordance:** a pinned "Dashboard" row at the **top** of the tree pane, mirroring the established pinned "Settings" row at the bottom (the *Settings Entrypoint in Sidebar Footer* requirement). Selecting it sets the center pane back to the Dashboard; selecting an artifact (tree) or a commit (rail) replaces it. This reuses the existing single-`RenderTarget`, last-selection-wins model — no new floating chrome, consistent with the rule that no affordance lives in the window's top-right corner.
- **Alternatives rejected:** a dedicated full-window dashboard mode (heavier, occludes the tree/rail), a fourth always-on pane (the layout is already three panes; a fourth would crowd the rail), and a Settings-like boolean toggle that swaps the right pane (duplicates the pane-swap logic and breaks the single-render-target model).

### Global scope; the rail stays per-selection

The Dashboard aggregates across **all** registered workspaces — matching the tray badge's global framing — and gives per-repo context via a breakdown section rather than by scoping the whole surface to one repo.

- When the Dashboard is the active center-pane target, no tree node owning a repository is selected, so the commit-graph rail shows its existing empty/placeholder state. That is unchanged behaviour, not a new rule.
- A drill-into-one-repo dashboard was considered and deferred. The global view is the thing the request asked for ("central hub… overview"); per-repo drill-down is an obvious future layer but would multiply the surface and the IPC contract for little v1 benefit.

### Temporal data is mined from git, not persisted

The app today is a live mirror of current filesystem state — it keeps no memory of the past. The Dashboard's time axis (the activity chart, throughput, time-to-archive) needs a temporal source the app does not have. It is mined from **git history**, reusing the machinery the commit-graph rail already established.

- **Why git over a persisted event log:** an event log (recording `CacheEvent`s to disk) would be exact and could capture non-git signal, but it is empty until it accumulates — no history on launch day, no trends for weeks. Git gives real history immediately, and the app already shells out to it (`git.rs`, the commit-graph rail). A snapshot-only option (no chart at all) was rejected because the user explicitly wants the rich, time-aware surface.
- **Two git reads, both degrade to empty:**
  1. **Commits-per-day** — `git log --all --since=<window> --pretty=%aI` per repo, bucketed by calendar day in the viewer's local time zone (the same day-bucketing the commit-graph rail already does for its day separators). This reuses the existing `commit_log` discipline; a `--since`-bounded variant keeps the read cheap.
  2. **Change lifecycle** — one pass per repo: `git log --reverse --diff-filter=A --name-status -- openspec/changes/`. Every commit that *adds* a file under `openspec/changes/<id>/…` dates that change's **creation**; every commit that adds a file under `openspec/changes/archive/<id>/…` dates its **archive**. From those two dates come throughput (archives in the recent window) and average time-to-archive.
- **The single-pass insight (keeps it O(repos), not O(changes)):** a naïve implementation would run one `git log -- <path>` per change — dozens of invocations on a repo with a large archive (this repo already has 30+). Instead, one `--diff-filter=A --name-status` pass over the whole `openspec/changes/` subtree yields every change's creation and archive add-events in a single command; the aggregator parses change ids out of the file paths. `git.rs` already uses `--diff-filter` / `--name-status` (`diff_tree_lines`), so this is idiomatic.
- **Accepted cost — task burn-down is the expensive bit, so it is deferred.** Charting task completion over time would require reconstructing each historical `tasks.md` (a per-commit diff or `git show <sha>:<path>` per data point), which is an order of magnitude more git work than the add-event scan. The task rollup on the Dashboard is therefore a **current-state snapshot**, not a time series. Velocity/burn-down remains an obvious fast-follow (see *Open / deferred*).

### Aggregation lives in the core; the command is thin

A new `openspec-core/src/dashboard.rs` owns the aggregation: it takes the registry, the cached `WorkspaceView`s, and the git-mined activity/lifecycle data and produces a single `DashboardData` payload (all IPC types `#[serde(rename_all = "camelCase")]`). The Tauri `get_dashboard` command only calls it.

- **Why:** the project rule is explicit — watchers, parsers, registries, and git logic belong in `openspec-core` so they stay testable from `cargo test` without the GUI. The summary counts, per-repo rollups, day-bucketing, and lifecycle math are all pure transforms over inputs and are unit-tested there. The shell crate stays a thin wrapper.
- **What's reused vs new:** the summary cards, per-repo breakdown, and recent feed are computed from data already in the cache / `WorkspaceView`s (the same source `useWorkspaces` already holds on the client) — the global task rollup is just a sum the client could even do itself, but it is centralised in the aggregator for one coherent payload. Only the two git reads above are genuinely new.

### Reactivity: recompute on demand, refresh on existing events

The Dashboard is pull-based: the frontend fetches `get_dashboard` when the Dashboard becomes (or is) the active surface, and a `useDashboard` hook refetches on the existing `cache-updated` / `change-added` / `change-archived` / `graph-changed` events while the Dashboard is shown — mirroring how `useWorkspaces` and `useCommitGraph` already wire events.

- **Why pull, not push:** the badge count is cheap and worth keeping always-fresh (it already is). The Dashboard's git mining is heavier and only meaningful while the surface is visible, so computing it on demand — rather than maintaining a continuously-pushed aggregate even when the Dashboard isn't shown — avoids wasted work. This also keeps the change small: no new event type, no new watcher.
- **No new ambient signal.** The Dashboard deliberately adds nothing to the always-on layer: the tray badge, dock badge, and notification rules (fire only on add/archive) are untouched. This is the line that reconciles "go rich" with the product's quiet stance — the richness is gated behind viewing the home surface, not pushed at the user.

### Graceful degradation is the house style, reused

Every `git.rs` function already returns empty/`None` outside a repo or when `git` is missing, and callers treat that as "not a git repository." The Dashboard inherits this directly: a flat (non-git) workspace, or a repo where `git` is unavailable, contributes its change/task counts and per-entry breakdown but supplies no commits-per-day data and no lifecycle dates. The activity and lifecycle sections aggregate only what they get; with zero git-backed repos they render an empty/neutral state rather than erroring.

## Staging

The home-surface placement fixes *where* the Dashboard goes, not *how much* it shows. Work is staged so the surface is not gated on the git mining:

1. **Calm core** — the Dashboard surface and default-landing behaviour, the pinned entry, summary cards, per-repo breakdown, and recent feed. All from data already in the cache / `WorkspaceView`s; no new git. This alone replaces the empty home state.
2. **Activity chart** — the commits-per-day chart from a bounded `git log --all --since` read, bucketed by day.
3. **Lifecycle metrics** — throughput + average time-to-archive from the single-pass `--diff-filter=A` lifecycle scan.

## Open / deferred

- **Task burn-down / velocity over time** — needs historical `tasks.md` reconstruction (per-commit diffing); the heaviest git work, deferred. The current-state task rollup ships now.
- **"Needs attention" triage** — stale/diverged worktrees, artifact-gap (proposal-only) changes, nothing-touched-in-N-days. The signals already exist (`divergence`, `ArtifactStatus`, `modifiedAt`) but surfacing them is intentionally out of this change to keep scope tight.
- **Per-author activity** — `commit_log` already carries `author`; a per-contributor breakdown is a cheap future addition.
- **Drill-into-one-repo dashboard** — global only this round.
- **Richer visualisation** — the first increment renders the activity chart as a simple sparkline/bar strip, not an interactive charting library.

## What does not change

- The headless-core / Tauri-shell split: aggregation, git mining, and the `DashboardData` types live in `openspec-core`; the shell only wraps them in a command.
- The existing `WatcherManager` and its `openspec/`-subtree scope; no new watcher and no new `CacheEvent` variant.
- The tree-selection contract and the existing artifact / commit render targets — they are *extended* with a `dashboard` variant, not altered.
- The tray badge, dock badge, and notification behaviour (notify only on add/archive) — untouched; the Dashboard adds no ambient signal.
- The commit-graph rail's per-selection scope and its empty state when no git-backed node is selected.
- The read-only posture of the app and the `SelfWriteTracker` pipeline.
