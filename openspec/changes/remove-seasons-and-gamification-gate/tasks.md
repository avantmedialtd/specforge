> **Ordering note.** The schema's default task order is core → shell → frontend, chosen so the workspace stays green throughout. This change is a pure deletion, which inverts that: removing `seasons.rs` first would break every consumer at once. The groups below run the dependency graph in reverse — leaves first, core last — which serves the same goal. See design.md, *Peel outside-in, keeping the tree green*. Run `bun run build` once before the first `cargo test` in a fresh worktree, or the Tauri and web crates cannot build (design.md, Risks).

## 1. Frontend — React components

- [ ] 1.1 In `src/components/DashboardView.tsx`, delete the `SeasonPanel`, `SeasonRecapCard`, and season-recap dismissal state, the `season-tierup` banner and its `tierUp` state, and the season-scoped `<Leaderboard title="Leaderboard · this season">` instance. Keep the all-time `<Leaderboard>` (`dashboard`: *Per-Author Leaderboard for Shared Repositories*).
- [ ] 1.2 In `src/components/DashboardView.tsx`, delete the `career-rank` chip block that renders `season.career.label` (`dashboard`: *Permanent Career Tier Readout* removed).
- [ ] 1.3 In `src/components/DashboardView.tsx`, remove the `equippedDescriptor` binding and the `equipped` prop from `Identicon`, and delete the `finishClass` / `treatment-finish` class composition inside the `Identicon` component so the avatar renders plain (`dashboard`: *Developer Profile Surface*).
- [ ] 1.4 In `src/components/DashboardView.tsx`, delete the `const gamified = data.gamificationEnabled` binding and unwrap every block it guarded — the streak in `dashboard-hero-right`, the `TodayHaul` / `Heatmap` / `Leaderboard` fragment, `<CommitGarden>`, and `<Celebration>` — so each renders unconditionally (`dashboard`: *Unconditional Progress Layer*).
- [ ] 1.5 In `src/components/SettingsView.tsx`, delete `BadgeFinishesSection`, `FinishSwatch`, and the `{gamification && <BadgeFinishesSection />}` call site (`dashboard`: *Equipped Badge Treatments* removed).
- [ ] 1.6 In `src/components/SettingsView.tsx`, delete the gamification toggle row, its `useState`, its change handler, and the "Show the gamified progress layer" copy (`dashboard`: *Unconditional Progress Layer*, scenario *No control disables the layer*).
- [ ] 1.7 In `src/hooks/useCommitGarden.ts`, drop the `enabled` argument and the clear-when-disabled path so the hook always fetches; update its doc comment (`commit-garden`: *Per-Workspace Commit Graphs at the Dashboard Bottom*).

## 2. Frontend — API surface, types, styles

- [ ] 2.1 In `src/api.ts`, remove the `getGamificationEnabled`, `setGamificationEnabled`, `equipTreatment`, and `treatmentWardrobe` wrappers and their doc comments.
- [ ] 2.2 In `src/types.ts`, remove `gamificationEnabled`, `season`, `seasonLeaderboard`, `recap`, `locker`, and `equipped` from the `DashboardData` interface — the hand-mirrored counterpart to the Rust field removals in task 6.1, which must land in the same change to keep the IPC boundary matched.
- [ ] 2.3 In `src/types.ts`, delete the now-unreferenced `TreatmentDescriptor`, `TreatmentEffect`, `Rarity`, `Wardrobe`, `SeasonStanding`, `SeasonRecap`, `BandTier`, `SeasonObjective`, and career types, plus the "Progress layer (gamified)" section comment.
- [ ] 2.4 In `src/App.css`, delete the `.finishes-*`, `.treatment-*`, `.identicon.treatment-finish`, `.career-rank*`, `.season-*`, and `.finishes-item:focus-visible` rule blocks. Verify each prefix with `grep -r "<prefix>" src/` returning zero hits before deleting, since no compiler catches an orphaned CSS rule (design.md, Risks).
- [ ] 2.5 Run `bun run build` and resolve any `noUnusedLocals` / `noUnusedParameters` errors surfaced by the deletions; the strict `tsc --noEmit` pass is the completeness check for groups 1–2.

## 3. Desktop shell (`crates/specforge`)

- [ ] 3.1 In `crates/specforge/src/commands.rs`, delete the `get_gamification_enabled`, `set_gamification_enabled`, `equip_treatment`, and `treatment_wardrobe` command handlers.
- [ ] 3.2 In `crates/specforge/src/lib.rs`, remove the four corresponding entries from the `invoke_handler!` generated list.

## 4. Web server (`crates/specforge-web`)

- [ ] 4.1 In `crates/specforge-web/src/dispatch.rs`, delete the `get_gamification_enabled`, `set_gamification_enabled`, `equip_treatment`, and `treatment_wardrobe` match arms and the settings comment banner naming gamification. Unknown commands already fall through to the structured-unsupported response (`web-ui`: *Command Transport Mirrors the In-Process Command Surface*).

## 5. Terminal frontend (`crates/specforge-tui`)

- [ ] 5.1 In `crates/specforge-tui/src/app.rs`, delete the `Screen::Season` variant, its `KeyCode::Char('3')` binding, and its arm in the screen-dispatch `match`.
- [ ] 5.2 In `crates/specforge-tui/src/app.rs`, rebind the surviving screens to a contiguous run — Browse `1`, Dashboard `2`, Garden `3`, History `4`, Settings `5` (`terminal-ui`: *Master-Detail Browse and Screen Navigation*, scenario *Screen keys are contiguous*).
- [ ] 5.3 In `crates/specforge-tui/src/app.rs`, remove `Model::gamification_on`, its initialiser, the Settings-screen re-read of `settings.gamification_enabled()`, and the toggle action that flips it.
- [ ] 5.4 In `crates/specforge-tui/src/ui.rs`, delete the `season()` screen renderer, the tier-ladder scroll region and its `TIER_COUNT` arithmetic, the `Screen::Season` draw and title arms, and the Season section of the Dashboard screen.
- [ ] 5.5 In `crates/specforge-tui/src/ui.rs`, delete the `if d.gamification_enabled` branch and the three "Enable gamification in SpecForge…" placeholder states so Dashboard and Garden always render their content (`terminal-ui`: *Progress Surfaces in the Terminal*, scenario *Progress surfaces need no opt-in*).
- [ ] 5.6 In `crates/specforge-tui/src/ui.rs`, remove the `("Gamification", model.gamification_on)` row from the `toggles` array, decrement `SETTINGS_TOGGLE_COUNT`, and fix the stale "after the two toggles" comment on `appearance_idx` — the constant positions the Appearance control, so it must move with the row (`terminal-ui`: *Settings Screen*).
- [ ] 5.7 In `crates/specforge-tui/src/ui.rs`, update the key legend line (`"1 / 2 / 3    Browse / Dashboard / Season"`) and any help text to the new five-screen numbering.
- [ ] 5.8 In `crates/specforge-tui/src/render_tests.rs`, delete `renders_gamified_screens_with_real_dashboard` and the gamification-flip persistence assertions, and re-index the Settings toggle-row assertions that addressed gamification as row 0.
- [ ] 5.9 In `crates/specforge-tui/README.md`, update the documented key legend to match task 5.2.

## 6. Application service (`crates/openspec-app`)

- [ ] 6.1 In `crates/openspec-app/src/settings.rs`, remove the `gamification_enabled` field, the `season: SeasonState` field, the `SeasonState` struct, the `gamification_enabled()` / `set_gamification_enabled()` accessors, and both `Default` initialisers. Add no migration — absent and orphaned keys are handled by `#[serde(default)]` and serde's unknown-field tolerance (`dashboard`: *Unconditional Progress Layer*, scenario *A legacy preference is ignored*).
- [ ] 6.2 In `crates/openspec-app/src/settings.rs`, remove the persistence test that flips `set_gamification_enabled` and asserts it reloads, and add a test proving a settings file containing legacy `gamificationEnabled` and `season` keys deserializes successfully and round-trips without them.
- [ ] 6.3 In `crates/openspec-app/src/service.rs`, delete the treatment-wardrobe accessor and the `Wardrobe` struct.
- [ ] 6.4 In `crates/openspec-app/src/service.rs`, remove the `if !self.settings.gamification_enabled() { return Ok(Vec::new()) }` guard from `commit_garden()` so the garden is always computed (`commit-garden`: *Per-Workspace Commit Graphs at the Dashboard Bottom*, scenario *Section needs no opt-in*).
- [ ] 6.5 In `crates/openspec-app/src/service.rs`, collapse the dashboard-assembly gamification branch: delete the season standing, season leaderboard, recap, locker, equipped, and career computation along with the `data.gamification_enabled = true` assignment, so the payload is built unconditionally from the analytics plus the progress layer.
- [ ] 6.6 In `crates/openspec-app/tests/dashboard.rs`, rewrite the two gamification tests so they assert the payload always carries the progress layer (streak, heatmap, today counts) and no longer reference a season standing or the enable/disable flag.

## 7. Core (`crates/openspec-core`)

- [ ] 7.1 In `crates/openspec-core/src/dashboard.rs`, remove the `season`, `season_leaderboard`, `recap`, `locker`, `equipped`, and `gamification_enabled` fields from `DashboardData` and their initialisers in `compute_dashboard`. This is the Rust half of the IPC mirror completed in task 2.2.
- [ ] 7.2 In `crates/openspec-core/src/dashboard.rs`, delete the public `season_baseline()` function only. Keep the private `trailing_avg_centi()` and `commits_trailing_avg_centi()` helpers — they still back the Today's-Progress average comparison, and removing them breaks the hero's comparison indicators with no type error (design.md, Risks).
- [ ] 7.3 Confirm the today-versus-average unit tests in `crates/openspec-core/src/dashboard.rs` still pass unchanged, as the guard on task 7.2 (`dashboard`: *Today's Progress Hero*, scenario *Comparison to recent daily average*).
- [ ] 7.4 In `crates/openspec-core/src/activity_log.rs`, remove the me-scoped season-window query and its doc reference to `seasons::season_window` (`activity-log`: *Bounded, Time-Bucketed Queries*).

## 8. Delete the seasons module

- [ ] 8.1 Delete `crates/openspec-core/src/seasons.rs` in full, including its in-file `#[cfg(test)]` module.
- [ ] 8.2 In `crates/openspec-core/src/lib.rs`, remove `pub mod seasons;` and the entire `pub use seasons::{…}` re-export block. A clean `cargo check` here is the completeness proof that no consumer was missed (design.md, *Peel outside-in*).

## 9. Vocabulary rename (`gamified` → `progress`)

- [ ] 9.1 In `crates/openspec-core` and `crates/openspec-app`, rename the concept in doc comments and identifiers — `ProgressData`'s "gamified progress layer" doc, `dashboard.rs`'s section comments, and `service.rs`'s assembly comments.
- [ ] 9.2 In `crates/specforge-tui/src/ui.rs` and `crates/specforge-tui/src/app.rs`, update the module docs that describe "the gamified screens" to the surviving progress surfaces.
- [ ] 9.3 In `src/types.ts`, `src/api.ts`, `src/components/DashboardView.tsx`, and `src/hooks/useCommitGarden.ts`, update doc comments referring to the gamified layer or its enable flag.
- [ ] 9.4 Run `grep -ri "gamif" crates/ src/ --include="*.rs" --include="*.ts" --include="*.tsx"` and confirm zero hits outside `openspec/changes/archive/`.

## 10. Verification

- [ ] 10.1 Run `bun run build` — strict `tsc --noEmit` plus bundle — and confirm it passes with no unused-local or unused-parameter errors.
- [ ] 10.2 Run `cargo test` for the workspace and confirm all suites pass, including the rewritten `openspec-app/tests/dashboard.rs` and the re-indexed `specforge-tui` render tests.
- [ ] 10.3 Run `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`; add assertions for any survivor in the rewritten `openspec-app` assembly rather than excluding it (design.md, Risks).
- [ ] 10.4 Run `grep -rn "season\|Season\|treatment\|Treatment\|career\|battle" crates/ src/ --include="*.rs" --include="*.ts" --include="*.tsx" --include="*.css"` and confirm every remaining hit is unrelated (e.g. CSS "treatment" styling, `--line` season-free output), with no live reference to the deleted system.
- [ ] 10.5 Start the app yourself with `bun run wt:dev` (never ask the user to run it) and walk the scenarios: on a fresh config the Dashboard shows Today's Progress, streak, heatmap, and the commit garden with no opt-in; Settings offers no gamification toggle and no Badge finishes section; the hero avatar is a plain identicon with no finish or rank chip.
- [ ] 10.6 With the app running, confirm a settings file that still contains `gamificationEnabled` and a populated `season` block loads without error and no longer contains those keys after the next settings write (`dashboard`: *Unconditional Progress Layer*, scenario *A legacy preference is ignored*).
- [ ] 10.7 Launch `specforge-tui` and confirm keys `1`–`5` each activate a screen with no dead key, the legend matches the bindings, and the Settings screen shows only the two quota toggles with the Appearance control correctly positioned below them (`terminal-ui`: *Master-Detail Browse and Screen Navigation*, *Settings Screen*).
