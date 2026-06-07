# Tasks

## 1. Remove milestone computation from the Rust core

- [x] 1.1 Delete `compute_milestones`, the `Milestone` struct, and the `TASK_MILESTONES` / `SHIP_MILESTONES` / `STREAK_MILESTONES` tables from `crates/openspec-core/src/dashboard.rs`
- [x] 1.2 Remove the `milestones` field from `ProgressData` and its assignment in `compute_progress`
- [x] 1.3 Drop `Milestone` from the `crates/openspec-core/src/lib.rs` public re-exports
- [x] 1.4 Remove the two milestone unit tests (`progress_milestones_cross_on_cumulative_totals`, `progress_backfilled_milestone_is_flagged`)

## 2. Remove the Milestones panel from the frontend

- [x] 2.1 Delete the `Milestones` component and the `milestoneGlyph` helper from `src/components/DashboardView.tsx`
- [x] 2.2 Remove the panel's render in `dashboard-grid` and the now-unused `Milestone` type import
- [x] 2.3 Remove the `Milestone` interface and the `milestones` field on `ProgressData` from `src/types.ts`

## 3. Re-home the equipped treatment finish onto the avatar

- [x] 3.1 Extend the hero `<Identicon>` to accept the equipped `TreatmentDescriptor` and apply the finish classes (`treatment-finish treatment--{effect} treatment--{rarity}`) and the `--treat-hue` / `--treat-hue2` vars when a treatment is equipped and gamification is on
- [x] 3.2 Add `.identicon.treatment-finish` CSS (glow + rim keyed by rarity, texture as a restrained inner wash); confirm the outer glow is not clipped by the identicon's `overflow: hidden`
- [x] 3.3 Remove the milestone CSS: `.dashboard-milestones`, `.milestone-row`, `.milestone-glyph`, `.milestone-label`, `.milestone-time`, and the `.milestone-glyph.treatment-finish` / `.milestone-glyph.treatment--legendary` finish rules

## 4. Reword copy that referenced milestone badges

- [x] 4.1 Update the `src/components/SettingsView.tsx` gating blurb and the locker helper text so "milestone badges" reads as "your avatar"

## 5. Verify

- [x] 5.1 `bun run build` (strict `tsc --noEmit` + bundle) is clean
- [x] 5.2 `cargo test --workspace` passes with no warnings
- [x] 5.3 `cargo fmt --check` and `cargo clippy` are clean
- [ ] 5.4 Visually confirm in dev: the Milestones panel is gone and the equipped finish renders on the avatar; tune the finish loudness (aura vs. rim). _Requires gamification enabled + a treatment equipped to see the finish; surfaces in the hot-reloading dev app._

## 6. Sync specs on archive

- [x] 6.1 Sync the `dashboard`, `seasons`, and `activity-log` deltas to the main specs
