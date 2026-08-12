# Remove the Season System and the Gamification Gate

## Why

The gamified layer arrived behind an opt-in switch that defaults to **off**, which means the app's most characterful surfaces — the streak, the contribution heatmap, the leaderboard, the commit garden, the ship confetti — are invisible unless a user goes looking for a toggle in Settings. That gate has outlived its purpose: these surfaces are simply what the Dashboard *is*, and hiding them behind a preference makes the default experience a bare analytics table.

The switch also bundles two very different things. On one side sit views derived directly from work the user actually did — days active, tasks completed, changes shipped, commits landed. On the other sits a **season system**: a monthly battle pass with a 30-tier ladder, adaptively-paced completion totals, rotating generated objectives, a treatment vault with rarity tiers, season recaps, and a locker of cosmetic **badge finishes** worn over the profile avatar. That second half is invented scoring — a game layered on top of the record rather than a reading of it. It is the largest single module in the core (`seasons.rs`, 1232 lines), it is the reason the settings file persists a locker and a rollover bookmark, and it is the only thing in the app that asks the user to care about a number that resets every month.

So: delete the gate and delete the game. What survives is the honest half — permanent, unconditional, and no longer optional.

## What Changes

- The **gamification master switch** is removed. The `gamificationEnabled` setting, its getter/setter, its two IPC commands, its web-dispatch arms, and its TUI toggle row all go. The surviving layer is unconditional.
- **Permanently included, no longer gated:** the Today's Progress hero, the streak, the contribution heatmap, the per-author leaderboard, the commit garden, and the live celebrations (ship confetti and the quieter task acknowledgement). Their behaviour is unchanged; only the gate is gone. `prefers-reduced-motion` remains the sole suppressor of motion.
- **Permanently removed — the season / battle-pass system:** the monthly season model and its deterministic naming, the two-track progression, season scoring, the 30-tier ladder and named bands, adaptive pacing and the entry baseline, the overflow lane, rotating generated objectives, season rollover and recaps, the vault, silent backfilled seasons, the season home on the profile band, the seasonal leaderboard variant, and the live tier-up acknowledgement. The whole `seasons` capability is retired.
- **Permanently removed — the badge finishes:** procedural badge treatments, rarity, the treatment locker, the equip action, the Settings *Badge finishes* section, and the equipped finish worn over the profile avatar. The identicon reverts to its plain, deterministic form.
- **The permanent career tier goes with them.** Although it is the never-resets counterpart to the seasonal band, it is still an invented ladder over lifetime totals, and it is the last consumer of `seasons.rs`. Removing it lets the module be deleted whole rather than leaving a vestigial scoring file behind. The `◆ <rank>` chip disappears from the Dashboard hero.
- **Vocabulary follows the code.** With no complement left, "gamified layer" / "gamification" no longer names anything meaningful. The concept is renamed to the **progress layer** throughout — spec requirement names, Rust doc comments, TypeScript type docs, and TUI module docs.
- **The terminal frontend loses its Season screen** and renumbers its remaining screens contiguously: Browse `1`, Dashboard `2`, Garden `3`, History `4`, Settings `5`. Its Settings screen loses the gamification toggle row; the Claude and ChatGPT quota toggles remain.
- **No settings migration is required.** Nothing in the workspace uses `serde(deny_unknown_fields)`, so an existing `settings.json` carrying `gamificationEnabled` and a populated `season` block deserializes straight past both, and the next write drops the orphaned keys. Unlocked treatments are discarded silently, which is the intent.

## Capabilities

### New Capabilities

<!-- none -->

### Removed Capabilities

- `seasons`: retired in full. All ten requirements — the season model and naming, two-track progression, season score derivation, the battle-pass ladder and bands, adaptive pacing, rotating objectives, procedural badge treatments, the locker/equipping/vault, silent backfilled seasons, and rollover/recap — are removed, and the capability's spec file is deleted on sync.

### Modified Capabilities

- `dashboard`: five requirements are removed — **Season Home on the Profile Band**, **Permanent Career Tier Readout**, **Equipped Badge Treatments**, **Seasonal Leaderboard Variant**, and **Live Tier-Up Acknowledgement**. **Gamification Opt-In** is replaced by a new **Unconditional Progress Layer** requirement asserting that the layer is always present and no setting gates it. **Personal Gamified Frame** is renamed **Personal Progress Frame** and loses its season-lens clause. **Today's Progress Hero**, **Developer Profile Surface**, **Per-Author Leaderboard for Shared Repositories**, and **Dashboard Unaffected by Workspace Disable** are modified to drop their season, treatment, and career-tier references.
- `commit-garden`: the section is no longer gated by the gamification opt-in — it is an unconditional part of the Dashboard. The purpose statement and the **Per-Workspace Commit Graphs at the Dashboard Bottom** and **Person-Colored Nodes** requirements drop their gating and season-scoring clauses.
- `terminal-ui`: the **Gamified Surfaces in the Terminal** requirement is renamed **Progress Surfaces in the Terminal** and loses the season standing and battle-pass ladder. **Master-Detail Browse and Screen Navigation** drops the Season screen and fixes the screen numbering. **Run Modes** drops the season standing from the `--line` output. **Settings Screen** drops the gamification toggle. **Read-Only Operation** drops its season reference.
- `activity-log`: **Bounded, Time-Bucketed Queries** drops the calendar-month (season) window query and the career-totals derivability clause; per-day bucketing over a bounded window is all that remains required. The "no new event kind" invariant is generalised from seasons to any derived view.
- `workspace-registry`: **Disabled Workspaces Continue To Be Watched** drops the season score from the list of things a disabled workspace's achievements still feed; the streak and heatmap clauses are unchanged.

## Impact

**Rust core (`crates/openspec-core`)**

- `src/seasons.rs` — **deleted** (1232 lines).
- `src/lib.rs` — drop `pub mod seasons` and the whole `pub use seasons::{…}` re-export block.
- `src/dashboard.rs` — remove the `season`, `season_leaderboard`, `recap`, `locker`, `equipped`, and `gamification_enabled` fields from `DashboardData` and their initialisers in `compute_dashboard`; delete the public `season_baseline()` helper. **Keep** the private `trailing_avg_centi()` / `commits_trailing_avg_centi()` helpers — they still back the Today's-Progress average comparison, and over-deleting them is the one real trap in this change.
- `src/activity_log.rs` — remove the season-window query and its doc reference to `seasons::season_window`.

**Application service (`crates/openspec-app`)**

- `src/settings.rs` — remove `AppSettings::gamification_enabled`, `AppSettings::season`, the `SeasonState` struct, `gamification_enabled()` / `set_gamification_enabled()`, and their `Default` initialisers plus the persistence test that flips the flag.
- `src/service.rs` — delete the treatment-wardrobe accessor and the `Wardrobe` type; drop the gamification early-return in `commit_garden()`; collapse the gamified branch in the dashboard assembly (the season standing, season leaderboard, recap, locker, equipped, and career computation) so the payload is built unconditionally from the analytics plus the progress layer.
- `tests/dashboard.rs` — rewrite the two gamification tests: the payload now always carries the progress layer and never a season standing.

**Tauri shell (`crates/specforge`)**

- `src/commands.rs` — delete `get_gamification_enabled`, `set_gamification_enabled`, `equip_treatment`, and `treatment_wardrobe`.
- `src/lib.rs` — drop the four entries from the `invoke_handler` list.

**Web server (`crates/specforge-web`)**

- `src/dispatch.rs` — delete the four matching dispatch arms and the settings comment banner that names gamification.

**Terminal frontend (`crates/specforge-tui`)**

- `src/app.rs` — delete `Screen::Season`, its key binding and key handler; renumber Garden/History/Settings to `3`/`4`/`5`; remove `Model::gamification_on` and the settings re-read of it; delete the gamification toggle action.
- `src/ui.rs` — delete the `season()` screen renderer and the tier-ladder scroll region; remove the gamification branch and the three "Enable gamification in SpecForge…" empty states; drop the Settings gamification row; update the help/key legend to the new numbering.
- `src/render_tests.rs` — drop the gamified-screens-with-real-standing test and the gamification-flip persistence test; re-index the Settings toggle-row assertions.
- `README.md` — update the key legend.

**Frontend**

- `src/components/DashboardView.tsx` — delete `SeasonPanel`, `SeasonRecapCard`, the tier-up banner, the seasonal `Leaderboard`, the career-rank chip, the `equippedDescriptor` binding and the `Identicon` `equipped` prop; remove the `gamified` flag and unwrap every block it guarded.
- `src/components/SettingsView.tsx` — delete `BadgeFinishesSection`, `FinishSwatch`, the gamification toggle row and its `useState` / handler.
- `src/hooks/useCommitGarden.ts` — drop the enabled argument; the hook always fetches.
- `src/api.ts` — remove `getGamificationEnabled`, `setGamificationEnabled`, `equipTreatment`, and `treatmentWardrobe`.
- `src/types.ts` — remove `gamificationEnabled` and the five season fields from `DashboardData`, plus `TreatmentDescriptor`, `TreatmentEffect`, `Rarity`, `Wardrobe`, `SeasonStanding`, `SeasonRecap`, and the career type.
- `src/App.css` — remove the treatment swatch, rarity, locker-grid, `.finishes-*`, `.treatment-*`, `.career-rank`, `.season-*` rule blocks.

**Specs**

- `openspec/specs/seasons/spec.md` — deleted on sync.
- `openspec/specs/{dashboard,commit-garden,terminal-ui,activity-log}/spec.md` — synced from the deltas on archive.

**Unaffected:** the activity log's event kinds and backfill, git mining, the identity and named-people roster, the commit-graph rail, the tray badge, notifications, the workspace registry and disable semantics, the quota status lines, and the web server's trust boundary. No new event kinds, no git behaviour change, no persisted-state migration.
