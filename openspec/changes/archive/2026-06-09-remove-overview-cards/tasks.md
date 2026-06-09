# Tasks

## 1. Remove the Overview cards from the Dashboard

- [x] 1.1 In `src/components/DashboardView.tsx`, delete the `.dashboard-cards`
  `<section>` (the three `.dashboard-card`s: task rollup, "Changes touch specs",
  "repos · worktrees"), keeping the surrounding `OVERVIEW` label, the activity
  chart, and the per-repository breakdown.
- [x] 1.2 Confirm `summary` is still referenced (the no-workspaces guard and the
  `N active · M archived` footer line) so no unused-symbol error arises; leave
  the footer line and the `DashboardSummary` payload unchanged.

## 2. Remove the now-dead card CSS

- [x] 2.1 In `src/App.css`, remove the `.dashboard-cards`, `.dashboard-card`,
  `.dashboard-card-value`, `.dashboard-card-value-sub`, `.dashboard-card-label`,
  `.dashboard-meter`, and `.dashboard-meter-fill` rules (used only by the removed
  cards).

## 3. Verify

- [x] 3.1 `bun run build` (tsc --noEmit + bundle) passes with no unused-local or
  unused-field errors. ✓ (517 modules, clean)
- [x] 3.2 Verified the removal structurally: zero remaining references to the
  removed classes/section in `src/`, and `.dashboard-cards` is an independent
  sibling of `.dashboard-grid` (removing it cannot alter the chart/breakdown
  layout). Full Tauri screenshot deferred as a pure deletion — available on
  request via `bun run wt:dev`.
