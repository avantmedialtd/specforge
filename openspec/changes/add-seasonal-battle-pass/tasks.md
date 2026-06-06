# Tasks

## 1. Season core foundation (`seasons` — `openspec-core`)

- [ ] 1.1 Add `crates/openspec-core/src/seasons.rs` with the season-index/window primitives: `season_index(year, month) = year*12 + (month-1)`, the current-season resolver from the viewer's local time zone, and the half-open `[month_start, next_month_start)` window — reusing the existing local-day bucketing
- [ ] 1.2 Add the deterministic season name generator: `season_name(season_index)` composing an adjective+noun from fixed word lists indexed by the season index (no per-season authoring); unit-test stability (same index → same name)
- [ ] 1.3 Add the weighted score economy as named constants + `season_score(window, me)`: sum Me-scoped `ChangeArchived`/`ArtifactReached`/`TaskCompleted`(×magnitude)/`ChangeCreated` + active-day credit from the activity log **and** Me-authored commits from the existing commit mining; assert no new event kind is introduced
- [ ] 1.4 Add the band/tier ladder: fixed tier count grouped into named bands (`Bronze→Silver→Gold→Platinum→Diamond→Master`, distinct from career-tier labels); map a score → `{ tier, band, gap_to_next }`
- [ ] 1.5 Add adaptive pacing: scale the total completion score to the developer's trailing baseline (reuse the recent-daily-average) × days-in-month, **clamped** between a floor and ceiling, distributed across the fixed tiers, with an unbounded **overflow** lane past the final tier
- [ ] 1.6 Add the objective archetypes (volume, cadence, streak, burst, breadth, finish, comeback) and `season_objectives(season_index, baseline)`: deterministic selection of K objectives rotating so an archetype never recurs back-to-back, with baseline-scaled thresholds
- [ ] 1.7 Add `objective_progress(objective, window, me)` deriving progress from in-window activity, marking completion, and granting bonus season score
- [ ] 1.8 Add the treatment descriptor generator `treatment(season_index, tier_index) -> TreatmentDescriptor { id, rarity, palette_refs, effect_kind, generator_version }` — deterministic, rarity rising with tier, `generator_version` baked into `id`; **no artwork/CSS in core**, descriptor data only
- [ ] 1.9 Add the permanent `career_tier(cumulative_totals)` derivation (monotonic from lifetime totals) and the deterministic `vault(season_index)` rotation that selects past-season treatments as currently earnable
- [ ] 1.10 Add `season_recap(window, me)` synthesising shipped/best-streak/band-reached/objectives-completed/treatments-unlocked for a season
- [ ] 1.11 Re-export the season surface from `lib.rs`; unit-test determinism (index/name/objectives/treatments), score monotonicity within a season, pacing floor/ceiling clamps, overflow, and career-tier monotonicity

## 2. Activity-log season queries (`activity-log` — `openspec-core`)

- [ ] 2.1 Add a calendar-month (season) window query helper over the append-only log, Me-scope-resolved, in the viewer's local time zone — sufficient to feed `season_score` and `objective_progress`; no schema change to `Achievement`
- [ ] 2.2 Unit-test season-window bucketing (events inside vs outside the month), Me-scope resolution, and that cumulative totals for milestone thresholds + career tier remain derivable

## 3. Locker + rollover persistence (shell — `specforge`)

- [ ] 3.1 Persist a `SeasonState` alongside `AppSettings` in `crates/specforge/src/settings.rs`: `{ unlocked: Set<TreatmentId>, equipped: Option<TreatmentId>, last_recapped_season_index }`, with serde defaults so an absent file loads empty; write only to the app data directory
- [ ] 3.2 Implement monotonic unlock (recompute may add, never revoke) and the equip setter; assert no write touches any workspace `openspec/` tree
- [ ] 3.3 On first launch, seed `last_recapped_season_index` to the current season so historical backfill does not fire per-month recaps

## 4. Dashboard aggregation wiring (`dashboard` — `openspec-core`)

- [ ] 4.1 Fold season state into the dashboard payload in `crates/openspec-core/src/dashboard.rs`: active season (name, window, countdown), band/tier + gap, the track + next unlock, objectives + progress, career tier, equipped treatment
- [ ] 4.2 Add the *This Season / All Time* lens parameter to the activity-log-derived aggregations (today/streak/heatmap/milestones), composing with the existing Me/Everyone scope, recomputing from the in-memory log without re-mining git
- [ ] 4.3 Add the season-scoped per-author leaderboard variant (windowed to the active season; omitted for single-author history) alongside the existing all-time leaderboard
- [ ] 4.4 Compute crossed tiers for the current season and unlock their treatments; flag **live** vs **backfilled** crossings so the shell can fire a live tier-up only for live ones; unlock backfilled-season treatments silently

## 5. Commands + IPC types

- [ ] 5.1 Add commands in `crates/specforge/src/commands.rs`: `get_season` (state + objectives + track + vault), `set_equipped_treatment`, and recap access; extend `get_dashboard` with the This-Season/All-Time lens; wire state in `lib.rs`
- [ ] 5.2 Mirror the IPC types in `src/types.ts` by hand (camelCase): season, band/tier, objective, treatment descriptor, locker, recap, and the lens enum — keep matched with the Rust `#[serde(rename_all = "camelCase")]` types
- [ ] 5.3 Wrap the new commands in `src/api.ts` via `invokeLogged`

## 6. Rollover detection wiring (shell — `specforge`)

- [ ] 6.1 In `crates/specforge/src/lib.rs`, detect a season boundary at launch and while running (checked on watcher ticks): when the active season index advances past `last_recapped_season_index`, mint the just-ended season's recap once and advance the bookmark
- [ ] 6.2 Ensure older/backfilled seasons are available in history without firing a live recap

## 7. Frontend — season home & lens (`DashboardView`)

- [ ] 7.1 In `src/hooks/useDashboard.ts`, thread the lens state and the new season payload; refetch on the existing cache/graph events
- [ ] 7.2 Render the **season home** on the profile band in `src/components/DashboardView.tsx`: name, end countdown, band/tier with gap-to-next, the battle-pass track strip with next-unlock preview, active objectives with progress, and the equipped treatment; encouraging zero state when empty
- [ ] 7.3 Render the permanent **career tier** distinctly from the seasonal band so the two tracks don't read as one
- [ ] 7.4 Add the **This Season / All Time** lens control (mirroring the Me/Everyone control) over the today/streak/heatmap/milestone views
- [ ] 7.5 Add the **recap** card surfaced once at rollover, and the **seasonal leaderboard** twin for multi-author repos
- [ ] 7.6 Add the live **tier-up acknowledgement** consistent with the existing celebration, suppressed under `prefers-reduced-motion`, non-blocking; no acknowledgement for backfilled crossings

## 8. Badge treatments — rendering & dev-time art pipeline

- [ ] 8.1 Implement treatment **rendering** in the desktop app (frontend): map a `TreatmentDescriptor` to a finish (token-palette tint + effect/overlay) applied **over** earned milestone badges, not replacing them; equip picker that lists unlocked treatments from the locker
- [ ] 8.2 Generate the build-time treatment **art** (base textures/overlays per rarity tier) by driving the **native ChatGPT desktop app** (per project preference — not Chrome, not an API); bake the assets into the bundle; confirm no runtime network fetch
- [ ] 8.3 Verify `generator_version` stability: a previously unlocked treatment renders identically after a hypothetical generator bump (snapshot/round-trip test of descriptor→render inputs)

## 9. Styling

- [ ] 9.1 Add styles in `src/App.css` for the season home, track strip, band/tier, objectives, equipped-treatment finishes, recap card, and the lens control; ensure reduced-motion variants for any animated finish

## 10. Verification

- [ ] 10.1 `cargo test` (workspace) green, including the new `seasons` unit tests
- [ ] 10.2 `bun run build` clean (tsc strict — `noUnusedLocals`/`noUnusedParameters` — plus the bundle)
- [ ] 10.3 Run the app (`bun run wt:dev` from this worktree's slot) and visually verify the season home, lens, treatment rendering, and the tier-up/recap moments; confirm the offline + read-only invariants hold
