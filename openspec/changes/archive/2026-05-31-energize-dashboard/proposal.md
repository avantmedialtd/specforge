# Energize the Dashboard with a gamified progress layer

## Why

The Dashboard answers "what is the state of everything?" — a calm, accurate snapshot of counts, a commits-per-day chart, and lifecycle averages. It does not answer "what did *I* accomplish, and am I on a roll?" Users open SpecForge to a static balance sheet; nothing rewards finishing a task or shipping a change, and there is no sense of daily achievement or momentum.

The app already *observes* every meaningful event — tasks getting checked off, artifacts reaching new statuses, changes being created and archived, commits landing — through its watcher and the git mining behind the commit-graph rail. But it discards the *when*, so it can show totals and never "today." Capturing that timeline turns the Dashboard from a snapshot into a narrative: what you achieved today, celebrated as it happens, with a sense of forward motion across days.

## What Changes

- Add a **Today's Progress** hero band: animated count-ups of what you accomplished today — tasks completed, changes shipped, commits landed, changes started — each with a comparison to your recent daily average.
- Add a **streak and contribution heatmap**: a consecutive-active-day streak counter and a GitHub-style multi-week calendar where today's cell is highlighted.
- Add **milestones and badges**: threshold achievements (first ship, task-count and ship-count thresholds, streak lengths) with a "recent milestones" panel, derived from cumulative activity.
- Add **live celebration moments**: confetti when a change is archived while the Dashboard is open, and a quieter glow when a task completes — both honoring `prefers-reduced-motion`.
- Add a new **activity-log** capability: an append-only event store persisted in the app's data directory (never inside `openspec/`) that records observed achievements with timestamps. Achievements are detected live by diffing the watcher's re-parses, and backfilled from git history over a bounded window so the views are populated on first launch.
- Demote — do not remove — the existing summary cards, per-repository breakdown, and lifecycle averages below the new progress band.

## Impact

- Affected specs: `dashboard` (new requirements + clarified read-only carve-out), `activity-log` (new capability)
- Affected code: `crates/openspec-core/src/activity_log.rs` (new), `crates/openspec-core/src/watcher.rs` (achievement detection on re-parse diff), `crates/openspec-core/src/dashboard.rs` (today / streak / heatmap / milestone aggregation), `crates/openspec-core/src/lib.rs`, `crates/specforge/src/commands.rs`, `crates/specforge/src/lib.rs`, `src/components/DashboardView.tsx`, `src/hooks/useDashboard.ts`, `src/App.tsx`, `src/api.ts`, `src/types.ts`, `src/App.css`
