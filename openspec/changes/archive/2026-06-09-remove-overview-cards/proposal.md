# Remove the Overview summary cards from the Dashboard

## Why

The "Overview" band shows three summary-metric cards — a task rollup
(`completed / total · %`), a "Changes touch specs" count, and a
"repos · worktrees" count. They add little day-to-day signal, take a full card
row at the top of the analytics block, and duplicate information already
available elsewhere (per-repository active/archived counts in the breakdown
below, and the active-change total in the footer). Removing them declutters the
Dashboard without losing anything the developer relies on.

## What Changes

- Remove the three Overview summary cards (`.dashboard-cards`) from the Dashboard.
- Keep the `OVERVIEW` label — it still heads the git-mined activity chart and the
  per-repository breakdown directly below it.
- Keep the bottom `N active · M archived` summary line; the active-change count is
  the one summary metric that survives, rendered there rather than as a card.
- No backend/IPC change: the dashboard payload still carries the now-unused
  rollup fields; only the UI stops presenting them.

## Capabilities

### Modified Capabilities

- `dashboard`: the *Cross-Workspace Summary Metrics* requirement is trimmed to
  the surviving active-change count (shown in the footer summary line); the task
  rollup, changes-touching-specs, and repository/worktree-count metric cards are
  no longer presented.

## Impact

- `src/components/DashboardView.tsx` — remove the `.dashboard-cards` `<section>`
  (the three `.dashboard-card`s). `summary` stays in use (the no-workspaces guard
  and the footer line), so no unused-symbol fallout.
- `src/App.css` — remove the now-dead `.dashboard-cards`, `.dashboard-card*`,
  `.dashboard-meter`, and `.dashboard-meter-fill` rules.
- No change to `crates/` or `src/types.ts`: the `DashboardSummary` payload is
  unchanged; the dropped fields simply go unconsumed by the UI (trimming them is
  a larger, separate refactor and is out of scope here).
