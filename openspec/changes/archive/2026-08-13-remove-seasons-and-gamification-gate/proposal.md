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

- `seasons`: retired in full — the `openspec/specs/seasons/` directory is **deleted outright** as an implementation task, retiring all eleven requirements: the monthly season model and deterministic naming, two-track progression, season score derivation, the battle-pass tier ladder and named bands, adaptive pacing with the overflow lane, rotating generated objectives, procedural badge treatments, the treatment locker/equipping/vault, silent backfilled seasons, season rollover and recap, and the capability's own read-only guarantee (whose surviving equivalents live in `dashboard`'s *Read-Only Operation* and `commit-garden`'s *Read-Only Graphs*).

  This change deliberately ships **no `specs/seasons/` delta**. A delta removing every requirement leaves the capability empty, and `openspec archive` then aborts the whole change with `Spec must have at least one requirement` — verified by running archive on a scratch copy. Deleting the directory directly is the only shape that archives cleanly with validation enabled, so the rationale for each retired requirement is recorded here rather than in a delta file.

### Modified Capabilities

- `dashboard`: five requirements are removed — **Season Home on the Profile Band**, **Permanent Career Tier Readout**, **Equipped Badge Treatments**, **Seasonal Leaderboard Variant**, and **Live Tier-Up Acknowledgement**. **Gamification Opt-In** is replaced by a new **Unconditional Progress Layer** requirement asserting that the layer is always present and no setting gates it. **Personal Gamified Frame** → **Personal Progress Frame**, **Per-Author Leaderboard for Shared Repositories** → **Per-Author Leaderboard**, and **Dashboard Unaffected by Workspace Disable** → **Dashboard Includes Disabled Workspaces** are renamed (removed and re-added), because each must drop a scenario and `openspec archive` rejects a MODIFIED block that drops one. **Today's Progress Hero**, **Developer Profile Surface**, and **Ship Selection Opens the Archive Browser** are modified in place — the last only to follow the renamed cross-reference.
- `commit-garden`: the section is no longer gated by the gamification opt-in — it is an unconditional part of the Dashboard. **Per-Workspace Commit Graphs at the Dashboard Bottom** is modified to drop the gating; **Person-Colored Nodes** is renamed **Person-Colored Graph Nodes** to drop its season-scoring scenario. The purpose statement is a manual post-archive edit — a delta's `## Purpose` section is not applied by `openspec archive`.
- `terminal-ui`: the **Gamified Surfaces in the Terminal** requirement is renamed **Progress Surfaces in the Terminal** and loses the season standing and battle-pass ladder. **Master-Detail Browse and Screen Navigation** drops the Season screen and fixes the screen numbering. **Run Modes** drops the season standing from the `--line` output. **Settings Screen** is renamed **Terminal Settings Screen** to drop the gamification toggle and its live-update scenario. **Read-Only Operation** drops its season reference. The purpose statement is a manual post-archive edit.
- `activity-log`: **Bounded, Time-Bucketed Queries** is renamed **Bounded, Per-Day Queries**, dropping the calendar-month (season) window query and the career-totals derivability clause; per-day bucketing over a bounded window is all that remains required. The "no new event kind" invariant is generalised from seasons to any derived view.
- `workspace-registry`: **Disabled Workspaces Continue To Be Watched** drops the season score from the list of things a disabled workspace's achievements still feed; the streak and heatmap clauses are unchanged.

## Impact

**Rust core (`crates/openspec-core`)**

- `src/seasons.rs` — **deleted** (1232 lines).
- `src/lib.rs` — drop `pub mod seasons` and the whole `pub use seasons::{…}` re-export block.
- `src/dashboard.rs` — remove the `season`, `season_leaderboard`, `recap`, `locker`, `equipped`, and `gamification_enabled` fields from `DashboardData` and their initialisers in `compute_dashboard`; delete the public `season_baseline()` helper **and its two orphaned unit tests**, after first adding a `compute_progress` test that asserts the today-versus-average fields directly. **Keep** the private `trailing_avg_centi()` / `commits_trailing_avg_centi()` helpers — they still back the Today's-Progress average comparison, and over-deleting them is the one real trap in this change.
- `src/activity_log.rs` — remove the `query_between_scoped()` season-window query, its doc reference to `seasons::season_window`, and its orphaned unit test.

**Application service (`crates/openspec-app`)**

- `src/settings.rs` — remove `AppSettings::gamification_enabled`, `AppSettings::season`, the `SeasonState` struct, their `Default` initialisers, and all six accessors: `gamification_enabled()`, `set_gamification_enabled()`, `season_state()`, `unlock_treatments()`, `set_equipped_treatment()`, and `set_last_recapped_season()`. The blanket `every_setter_round_trips_and_every_getter_reads_back` test is **edited, not deleted** — it is the file's only mutation coverage for ~20 unrelated settings.
- `src/service.rs` — delete the `treatment_locker()` accessor and the `TreatmentLocker` type; drop the gamification early-return in `commit_garden()`; collapse the gamified branch in the dashboard assembly (the season standing, season leaderboard, recap, locker, equipped, and career computation, plus the then-unused `settings_arc` binding) so the payload is built unconditionally from the analytics plus the progress layer.
- `tests/dashboard.rs` — rewrite the two gamification tests: the payload now always carries the progress layer and never a season standing.

**Tauri shell (`crates/specforge`)**

- `src/commands.rs` — delete `get_gamification_enabled`, `set_gamification_enabled`, `set_equipped_treatment`, and `get_treatment_locker`.
- `src/lib.rs` — drop the four entries from the `invoke_handler` list.

**Web server (`crates/specforge-web`)**

- `src/dispatch.rs` — delete the four matching dispatch arms, the `treatment_id` args field, and the comment banners naming treatments and gamification.

**Terminal frontend (`crates/specforge-tui`)**

- `src/app.rs` — delete `Screen::Season`, its key binding, key handler, and the `season_scroll` field; renumber Garden/History/Settings to `3`/`4`/`5`; remove `Model::gamification_on` and the settings re-read of it; delete the gamification arm of `toggle_focused_setting` **and renumber the two surviving arms**; decrement `SETTINGS_TOGGLE_COUNT` (defined here, not in `ui.rs`).
- `src/ui.rs` — delete the `season()` screen renderer, the tier-ladder scroll region, and the treatment-locker strip; remove the unconditional "Gamification: on/off" Dashboard header, the gamification branch, and the three "Enable gamification in SpecForge…" empty states; drop the Settings gamification row and the `rarity_word` / `rarity_style` helpers; update the help/key legend to the new numbering.
- `src/render_tests.rs` — drop the gamified-screens-with-real-standing test and the gamification-flip persistence test; re-index the Settings toggle-row assertions; rebind all nine `Char('6')` Settings-navigation presses to `Char('5')`.
- `src/theme.rs` — delete the `Rarity` import and the `Theme::rarity` method, and drop the rarity line from the Mono-scheme downsampling test.
- `README.md` (TUI) — update the key legend, run-mode table, screen descriptions, and settings-toggle copy.

**Frontend**

- `src/components/DashboardView.tsx` — delete `SeasonPanel`, `SeasonRecapCard`, the tier-up banner, the seasonal `Leaderboard`, the career-rank chip, the `equippedDescriptor` binding and the `Identicon` `equipped` prop; remove the `gamified` flag and unwrap every block it guarded.
- `src/components/SettingsView.tsx` — delete `BadgeFinishesSection`, `FinishSwatch`, the gamification toggle row and its `useState` / handler.
- `src/hooks/useCommitGarden.ts` — drop the enabled argument; the hook always fetches.
- `src/api.ts` — remove `getGamificationEnabled`, `setGamificationEnabled`, `setEquippedTreatment`, and `getTreatmentLocker`.
- `src/types.ts` — remove `gamificationEnabled` and the five season fields from `DashboardData`, plus `SeasonInfo`, `BandTier`, `SeasonObjective`, `Rarity`, `TreatmentDescriptor`, `TreatmentEffect`, `CareerTier`, `SeasonStanding`, `SeasonRecap`, and `TreatmentLocker`.
- `src/App.css` — remove the treatment swatch, rarity, locker-grid, `.finishes-*`, `.treatment-*`, `.career-rank`, `.season-*` rule blocks and the orphaned `@keyframes tierup-in`. Two **grouped** rules are edited rather than deleted: the fifteen-selector focus-ring rule (drop only `.finishes-item` and `.season-recap-close`) and the reduced-motion rule (drop only `.season-tierup`) — deleting either wholesale silently regresses focus rings and reduced-motion suppression app-wide.

**Documentation**

- root `README.md` — the TUI paragraph still says "Press `6` for a **Settings** screen that toggles gamification"; after renumbering the key is `5` and no such toggle exists. The mutation-stats paragraph also cites `seasons.rs` as the top survivor file.

**Specs**

- `openspec/specs/seasons/` — deleted during implementation (task 9.5), not via a delta; see *Removed Capabilities* above.
- `openspec/specs/{dashboard,commit-garden,terminal-ui,activity-log,workspace-registry}/spec.md` — synced from the deltas on archive. The `## Purpose` paragraphs of `terminal-ui` and `commit-garden` are **not** applied by archive and are hand-edited afterwards.

**Unaffected:** the activity log's event kinds and backfill, git mining, the identity and named-people roster, the commit-graph rail, the tray badge, notifications, the workspace registry and disable semantics, the quota status lines, and the web server's trust boundary. No new event kinds, no git behaviour change, no persisted-state migration.
