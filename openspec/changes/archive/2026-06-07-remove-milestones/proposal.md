# Remove Milestones

## Why

The *Milestones* panel is dysfunctional on both counts. **As a sense of achievement:** its thresholds are crossed off cumulative totals, so on first launch the git-history backfill silently crosses every threshold the developer's past already satisfies — the spec itself says a backfilled milestone "SHALL be shown as earned but SHALL NOT trigger a live celebration." A working developer opens the app to a panel pre-filled with retroactive trophies they never felt earning, and it then *freezes*, because the next rung (500 tasks, 50 changes, a 100-day streak) is months away. Flood, then frost. **As UI:** it is a flat emoji-glyph list (🎉🏆🔥🏅) sorted by recency, so the timestamp-less streak badges always sink to the bottom, and the equipped seasonal finish is painted over emoji.

But the panel is quietly load-bearing: the earned milestone badges are the **only Dashboard surface** that renders the seasonal battle pass's equipped treatment finish. Deleting the panel outright would orphan the just-shipped season cosmetics, leaving them visible only as a swatch in *Settings → Badge finishes*. So this change **re-homes** the finish rather than dropping it: the equipped treatment moves onto the developer's **profile avatar** (the identicon). That turns the achievement signal from an inert, backfilled list into a *live* mark on the developer's always-visible identity — one that shifts as they climb the season ladder — which is a better answer to "sense of achievement" than the panel ever was.

## What Changes

- The **Milestones panel** and its `Milestones` component are removed from the Dashboard. The `compute_milestones` function, the `Milestone` type, the three threshold tables (task / ship / streak), and the `milestones` field on `ProgressData` are removed from the Rust core and the TypeScript mirror, along with the milestone CSS and the two milestone unit tests.
- The **equipped treatment finish** is re-homed from the earned milestone badges onto the developer's **profile avatar** (the identicon) in the Dashboard hero. The locker and equipping flow in *Settings → Badge finishes* are unchanged; only the Dashboard render target moves.
- The **Developer Profile** highlight reel becomes the avatar (now wearing the equipped finish) plus the *Me*-scoped streak; "earned milestones" is dropped from the reel.
- No persisted-state migration: the treatment locker and equipped selection are untouched — only what the finish renders over changes. No new event kinds; no change to git mining or the activity log.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `dashboard`: the **Milestones and Badges** requirement is removed. **Equipped Badge Treatments** is modified so the finish renders over the profile avatar rather than earned milestone badges. **Developer Profile Surface** is modified so its highlight reel is the avatar-with-finish plus the streak, dropping earned milestones. **Gamification Opt-In** and **Personal Gamified Frame** are modified to drop milestones from their gated-layer and personal-frame enumerations (and to retarget the gated "equipped-treatment finish" from badges to the avatar).
- `seasons`: **Procedural Badge Treatments** is modified so a treatment is a finish applied over the developer's avatar (not a per-badge finish over a set of earned badges). **Silent Backfilled Seasons** is modified to state its silent-backfill principle directly rather than leaning on the now-removed "backfilled milestones" rule.
- `activity-log`: **Activity Event Log** and **Bounded, Time-Bucketed Queries** are modified only to drop their incidental references to the removed milestone views/thresholds (the log remains the source of truth for the today, streak, and heatmap views, and its cumulative totals still feed the permanent career tier). No behavioural change.

## Impact

- `src/components/DashboardView.tsx` — delete the `Milestones` component, the `milestoneGlyph` helper, the `Milestone` type import, and the panel's render in `dashboard-grid`; pass the equipped `TreatmentDescriptor` into the hero `<Identicon>` so the avatar wears the finish.
- `src/types.ts` — remove the `Milestone` interface and the `milestones` field from `ProgressData`.
- `crates/openspec-core/src/dashboard.rs` — remove `compute_milestones`, the `Milestone` struct, the `TASK_MILESTONES` / `SHIP_MILESTONES` / `STREAK_MILESTONES` tables, the `milestones` field on `ProgressData`, its assignment in `compute_progress`, and the two milestone unit tests.
- `crates/openspec-core/src/lib.rs` — drop `Milestone` from the public re-exports.
- `src/App.css` — remove the milestone rules (`.dashboard-milestones`, `.milestone-row`, `.milestone-glyph`, `.milestone-label`, `.milestone-time`, and the two `.milestone-glyph.treatment-finish` / `.milestone-glyph.treatment--legendary` finish rules); add an `.identicon.treatment-finish` rule (plus rarity variants) so the avatar carries the finish without the glow being clipped by the identicon's `overflow: hidden`.
- `src/components/SettingsView.tsx` — copy only: the gating blurb and the locker helper text that mention "milestone badges" are reworded to "your avatar."
- `openspec/specs/dashboard/spec.md`, `openspec/specs/seasons/spec.md`, `openspec/specs/activity-log/spec.md` — synced from the deltas on archive.
- **No change** to the streak, heatmap, today's haul, leaderboards, season ladder/objectives, career rank, celebrations, analytics, activity log, git mining, or the persisted treatment locker. No new event kinds; no persisted-state migration.
