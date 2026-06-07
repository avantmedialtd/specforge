# Remove Dashboard Toggles

## Why

The Dashboard hero carries two segmented controls — *Me / Everyone* scope and *This Season / All Time* lens — that fight the grain of the gamified frame. That frame is intrinsically first-person: *your* streak, *your* milestones, *your* flame. "Everyone's streak" is the union of unrelated authors' active days, owned by no one; "this season's streak" is an odd truncation of an inherently cumulative fact. The legitimate team and season views are already owned, and better expressed, elsewhere — the per-author **Leaderboard** (ranked, with a seasonal variant) and the **season home**. Both toggles already default to the personal, all-time view, so removing them is almost purely subtractive: a calmer, switch-free header with no change to what an untouched Dashboard shows.

## What Changes

- The **Me / Everyone scope** segmented control is removed. The gamified, activity-log-derived views (today's progress, streak, heatmap, milestones, and the in-flight tile) are now unconditionally resolved to the canonical developer — the prior default.
- The **This Season / All Time lens** segmented control is removed. The same views now always cover all available history; the season window lives only in the season home and the seasonal leaderboard, which compute it independently.
- The hero's `dashboard-toggles` container and both toggle components are removed; the per-author leaderboard, season home, season ladder, career rank, celebrations, and the neutral analytics are **untouched**.
- The in-flight active-change count, which the running default already computed as the developer's, is now the spec's single unambiguous definition (previously the base requirement read as the global count and the scope requirement qualified it to *Me*).
- The `get_dashboard` IPC command drops its `scope` and `lens` parameters; the `DashboardScope` and `DashboardLens` types are removed. No new event kinds, no git re-mining changes.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `dashboard`: the **Activity Scope Selection (Me / Everyone)** and **Season Lens (This Season / All Time)** requirements are removed. A new **Personal Gamified Frame** requirement carries the surviving invariant (the gamified views resolve unconditionally to the developer, with no widening or season-narrowing control) and rehomes the alias-claiming scenario. **Today's Progress Hero** is modified so its in-flight count is defined as the developer's active changes. **Gamification Opt-In** is modified to drop the two controls from the gated-layer enumeration.

## Impact

- `src/components/DashboardView.tsx` — delete the `ScopeToggle` and `LensToggle` components, the `scope`/`lens` `useState` hooks, and the `dashboard-toggles` wrapper; call `useDashboard()` with no args.
- `src/hooks/useDashboard.ts` — drop the `scope` and `lens` arguments.
- `src/api.ts` — `getDashboard()` loses both params; remove the `DashboardScope` / `DashboardLens` imports.
- `src/types.ts` — remove the `DashboardScope` and `DashboardLens` types.
- `crates/specforge/src/commands.rs` — `get_dashboard` drops the `scope`/`lens` params; `only_me` collapses to always-true and `season_lens` to always-false, deleting the *Everyone* branches (achievements, commit-days, in-flight) and the season-lens branch.
- `crates/openspec-core/src/dashboard.rs` — `compute_progress` stays scope-agnostic; the me-vs-everyone unit test is kept (it exercises `compute_progress` directly) and may be renamed for clarity now that no toggle reaches it.
- `src/App.css` — remove the `.dashboard-toggles`, `.scope-toggle`, and `.scope-toggle-btn` rules.
- `openspec/specs/dashboard/spec.md` — synced from the delta on archive.
- **No change** to the leaderboards, season surfaces, career rank, celebrations, analytics, activity log, or git mining. No new event kinds; no persisted-state migration.
