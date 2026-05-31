## Context

The Dashboard (added in `add-dashboard`) is a read-only aggregation surface computed on demand from the in-memory cache and the git mining path. It is deliberately stateless: nothing about *when* an achievement happened is retained, so it can show totals but never "today" or "this streak."

This change adds the missing time dimension. The app already emits the raw signals — the watcher re-parses each workspace on debounced batches and emits `CacheEvent::{Updated, ChangeAdded, ChangeArchived}`, and the commit-graph mining already yields per-day commit buckets and per-change creation/archival dates. What is missing is a *durable record* of these signals over time. We introduce an **activity log** (`activity_log.rs`) — an append-only event store in the app data directory — and grow the dashboard aggregation and presentation to render today's haul, a streak/heatmap, milestones, and live celebration on top of it.

The user explicitly chose the richest, most honest time source (a live event log backfilled from git) and the full gamified treatment (today's haul, streaks + heatmap, milestones, and live celebration).

## Goals / Non-Goals

### Goals

- Make the Dashboard feel like a highlight reel: show what was achieved *today*, celebrate completion as it happens, and convey momentum across days.
- Capture observed achievements durably without compromising the app's read-only relationship to workspaces.
- Populate the views on first launch via a bounded git backfill, so a new user does not see an empty, discouraging board.
- Reuse existing infrastructure: the watcher's re-parse diff, the commit-graph git mining, and the existing `change-archived` event (which already drives notifications) for live celebration.

### Non-Goals

- No mutation of workspace state — no editing, task toggling, or archiving from the Dashboard (the existing read-only invariant stands).
- No telemetry leaving the machine; the activity log is local-only.
- No per-task timestamps in the parser. Task completion *time* is inferred from the watcher diff (live) or git history (backfill), not stored per task line.
- No sound by default; no XP/leveling system in this change.

## Decisions

### Achievement events live in an app-data activity log, not in the workspace

The activity log is persisted in the application data directory next to `settings.rs`'s file — never inside any workspace's `openspec/` tree. This keeps SpecForge a read-only *viewer* of workspaces: it merely keeps its own private diary of what it observed. The dashboard's "Read-Only Operation" requirement is clarified (not weakened) to make this carve-out explicit.

### Detect achievements by diffing the watcher's re-parse

The cache already replaces a workspace's parsed `Vec<ChangeData>` on each debounced re-parse. We diff previous vs. new at replacement time: an increase in a change's `completedTasks` records a task-completed achievement (count = delta); an artifact reaching a new status records an artifact-reached achievement; a change appearing records a created achievement; a change moving to archived records a shipped achievement. This reuses the existing seam and needs no new file access.

### Net-positive only; unchecks and deletions do not subtract

Task counts can fall (a box gets unchecked, or a task line is deleted). History is append-only and never rewritten, so only net-positive deltas produce events. This keeps "tasks done today" monotonic and avoids a number that mysteriously drops.

### Git backfill is bounded and degrades without git

On first observation of a git-backed workspace (no prior log history), we backfill over a bounded window (~90 days): creation/archival dates from the commit that adds files under `changes/<id>/` and `archive/<id>/` (already mined for lifecycle metrics), commit activity from `git log`, and task completions by diffing `tasks.md` checkbox state across commits. The task-diff backfill is the heaviest piece and is windowed; it may land after the cheaper signals. Non-git (flat) workspaces contribute no backfill and rely on live capture going forward.

### Streak rule: any achievement (including a commit) sustains a day

A day counts toward the streak if it has at least one recorded achievement of any type — a checked task, a shipped change, or a commit. This makes the streak neither trivially easy nor punishingly hard. A day with zero recorded achievements breaks it.

### "vs average" baseline is the trailing 30 active days

Today's-haul comparison uses the mean of the last 30 *active* days for that achievement type, so a weekend of rest doesn't depress the bar.

### Milestones are derived, not separately persisted

Badge thresholds (first ship; 10/50/100 tasks; 5/10/25 ships; 3/7/30-day streaks) are computed from the activity log's cumulative totals, so unlock state is deterministic and needs no second store. Milestones satisfied *before* the log existed (recovered via backfill) are shown as earned but never trigger live celebration — otherwise first launch would fire a confetti barrage.

### Live celebration rides the existing event, gated by reduced motion

Confetti fires only on `ChangeArchived` (rare, meaningful) while the Dashboard is the active surface, reusing the same event that already drives desktop notifications. Task completions get a quieter glow. All motion (confetti and count-up animations) is suppressed under `prefers-reduced-motion`.

## Risks / Trade-offs

- **Cold start feels empty.** Git backfill mitigates this for git-backed repos; flat workspaces and brand-new repos get a warm "let's get going" empty state rather than a guilt-inducing `0 ▼`.
- **mtime vs. event-log timestamps.** The existing recent feed uses mtime (best-effort). Achievement timestamps are wall-clock at observation (live) or commit dates (backfill) — more honest, but a long-running edit session attributes a completion to when the watcher saw it, which is acceptable.
- **Task-diff backfill cost.** Diffing `tasks.md` across many commits can be expensive; it is bounded to the window and can be deferred behind the cheap signals. Silent truncation at the window edge is acceptable and documented.
- **Gamification taste.** Streaks can induce pressure. Framing stays gentle (no loss aversion beyond the streak number; no nagging), and confetti is reserved for genuine wins.

## Migration / Rollout

Additive. The activity log is created lazily in app data on first run; absence of a prior log triggers backfill. The Dashboard re-prioritizes its layout (progress band on top, existing analytics demoted below) without removing any existing metric. No workspace files are touched.
