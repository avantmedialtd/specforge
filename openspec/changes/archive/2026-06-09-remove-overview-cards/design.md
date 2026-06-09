## Context

The Dashboard's "Overview" band renders `.dashboard-cards`
(`DashboardView.tsx:996–1027`): three `.dashboard-card`s backed by
`summary.completedTasks/totalTasks/taskPercent` (task rollup),
`summary.specsTouching` (changes touch specs), and
`summary.repoCount/worktreeCount/flatCount` (repos · worktrees).

The `dashboard` spec's *Cross-Workspace Summary Metrics* requirement
(`dashboard/spec.md:33`) enumerates **four** aggregated metrics. Three of them
are these cards. The fourth — the total number of active (non-archived) changes
— is rendered separately in the bottom footer line
(`{summary.activeChanges} active · {totalArchived} archived`,
`DashboardView.tsx:1078`), which is not part of the Overview cards.

## Goals / Non-Goals

**Goals:**
- Remove the three Overview metric cards and their dead CSS.
- Keep the Dashboard otherwise intact: the `OVERVIEW` label, activity chart,
  per-repository breakdown, today's-ships feed, and the footer summary line.

**Non-Goals:**
- No backend/IPC refactor: the `DashboardSummary` payload keeps the rollup
  fields even though the UI no longer reads them.
- No change to the footer `N active · M archived` line.

## Decisions

**D1 — Keep the footer summary line; this is a MODIFY, not a REMOVE.** Only the
three cards (metrics 2–4) are removed; the active-change count (metric 1) lives
in the footer and stays. So the spec change trims *Cross-Workspace Summary
Metrics* down to the surviving active-change count rather than removing the
requirement. Alternative (drop the footer too and REMOVE the requirement) was
not chosen — the user pointed only at the cards, and the footer is a compact,
spatially-separate element.

**D2 — Retain the requirement name, so dependent references stay valid.** Because
the active-change count survives under the same *Cross-Workspace Summary
Metrics* requirement, the term "summary metrics" remains meaningful. The
*Graceful Degradation Without Git* requirement (still lists "summary metrics"
among non-git sections), the *Gamification Opt-In* requirement (lists
"cross-workspace summary metrics" among always-on analytics), and the spec's
Purpose prose therefore remain accurate and need **no** edit — the only delta is
the one MODIFIED requirement.

**D3 — UI-only removal; leave the backend computing the fields.** Trimming
`completedTasks/totalTasks/taskPercent/specsTouching/worktreeCount/flatCount`
from `DashboardSummary` would ripple into `dashboard.rs`, the hand-mirrored
`types.ts`, and Rust tests for no user-visible gain. The spec governs what the
Dashboard *presents*, not what the payload may carry, so the extra fields are
harmless. `repoCount`/`flatCount` are still read by the no-workspaces guard.

## Risks / Trade-offs

- The payload carries fields the UI no longer reads → minor, and a clean
  follow-up can trim them; spec/impl stay consistent because the spec is about
  presentation. → Mitigation: documented as an explicit non-goal.
- Removing the `.dashboard-meter*` rules → verified those classes are used only
  by the task-rollup card (no other consumer in `src/`).
