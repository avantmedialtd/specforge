# Tasks

## 1. Extract `openspec-app` (strangler-fig — desktop stays green throughout)

- [x] 1.1 Create `crates/openspec-app/`, depending on `openspec-core`; add to the workspace members
- [x] 1.2 Move `settings.rs` (`AppSettings`, `SeasonState`, `SettingsStore`) verbatim from `crates/specforge` into `openspec-app`; re-point the shell's imports — pure move, no behavior change
- [x] 1.3 Add `openspec_app::config_dir()` resolver reproducing Tauri's identifier-based path per-OS; switch the shell from `app.path().app_config_dir()` to it; verify the resolved path is unchanged on macOS
- [x] 1.4 Introduce `AppService` owning the state handles (registry, settings, presentation, activity log, watcher); add `AppService::bootstrap(config_dir)` absorbing the `lib.rs` setup + `backfill_activity` + identity/season seeding; have the shell `setup()` call it and `manage` the service
- [x] 1.5 Extract the `get_dashboard` body into `AppService::dashboard()`; reduce the `#[tauri::command]` to a delegate
- [x] 1.6 Add the `cargo test` for the dashboard assembly that was previously impossible — covers headless callability + season-target stability across reads (no mid-season drift)
- [~] 1.7 `AppService` gained the full read surface the TUI needs (dashboard, garden, graph, detail, diff, read_artifact, workspace_views, changes_for, list_workspaces, list/archived, active_count, treatment_locker); shell delegates dashboard/garden/treatment-locker. Full delegation of the remaining read + mutation commands is deferred polish (shell still operates them on the shared handles)
- [x] 1.8 Harden the season-recap bookmark advance to be idempotent (guards the same-machine two-reader rollover edge; benefits the desktop too)
- [x] 1.9 Confirm `cargo test` is green and the desktop app builds and behaves unchanged (workspace suite: openspec-app 2, openspec-core 163+, specforge 9 — all pass; `clippy -D warnings` + `fmt --check` clean)

## 2. `specforge-tui` skeleton (TEA loop + plumbing)

- [x] 2.1 Create `crates/specforge-tui/` depending on `openspec-app`; add `ratatui`, `crossterm` (`event-stream`), `pulldown-cmark`, `futures`; add to workspace members (used a hand-rolled tree instead of `tui-tree-widget` to avoid a ratatui-version coupling)
- [x] 2.2 `#[tokio::main]` entry: parse run mode (default / `--status` / `--line`); terminal raw mode + alternate screen with a panic hook that restores it
- [x] 2.3 Define `Model`, `Msg`, `update`, `view`; run the `tokio::select!` loop over crossterm `EventStream` + `AppService::subscribe()` + a slow tick
- [x] 2.4 Live refresh: on a `CacheEvent`, re-read `workspace_views()` from the service (no parallel cache); async loads (artifact, dashboard) run off-loop and post `Msg::Artifact`/`Msg::Dashboard` back through an mpsc channel
- [x] 2.5 Keymap and focus: screens via `1`–`5` + `Esc`, pane focus via `Tab`, vim motion, `?` help overlay, `q`/`Ctrl-c` quit, and a `/` incremental tree filter (case-insensitive substring over titles/names with parent-retention, matched substring underlined, `Enter` applies / `Esc` clears) mirroring the desktop archive search
- [x] 2.6 `theme` module: terminal-capability detection (truecolor / 256 / 16 / mono via `COLORTERM`/`TERM`/`NO_COLOR`) with a `PaletteColor` → RGB → downsample ladder, the desktop's 8 lane colours, FNV-1a per-person garden hues, rarity colours, and emoji/Unicode-vs-ASCII glyph gating (UTF-8 locale + non-dumb). Workspace tints wired onto tree headers
- [x] 2.7 Responsive layout: two-pane Browse above a width threshold, single focused pane below it

## 3. Views and signature widgets

- [x] 3.1 Workspace/change tree from `workspace_views()`, with disclosure, change-status glyphs (`○`/`◐`/`●`), and a 7-cell task-progress bar
- [x] 3.2 Markdown detail pane: `pulldown-cmark` → styled ratatui lines (headings, lists, emphasis, code spans/blocks; alt-text for images), now with `ENABLE_TASKLISTS` rendering `☑`/`☐` checkbox glyphs (completed lines dimmed + struck). Detail pane carries a present-only artifact tab bar (proposal · design · tasks · spec:&lt;cap&gt;), switched with `[`/`]`, each tab loaded async and the pane title made tab-aware
- [x] 3.3 Commit-graph rail (`CommitGraph`/`LaidOutCommit`/`EdgeSegment` → box chars) as the History screen (key `5`): per-lane verticals, fork/converge elbows (`╭╮╰╯┼`), lane colours matching the desktop, ref chips, selection highlight, and a `m`-to-load-more affordance when truncated
- [x] 3.4 Dashboard screen: rendered from the **typed** `DashboardData` — gamification flag, summary metrics, ships-today, the relative-intensity contribution heatmap (7-row week-column grid cropped to recent weeks that fit), streak, and both per-author leaderboards (shown only for a multi-author contest), plus a season teaser
- [x] 3.5 Season screen: full scrollable 30-tier battle-pass ladder — every tier's threshold, reward (`treatment()` effect + rarity colour), lock/current/unlocked glyphs and equipped marker — auto-scrolled to the current tier (Unranked and overflow handled), with a treatment-locker footer
- [x] 3.6 Commit garden screen (key `4`) from `Vec<WorkspaceGarden>`: per-workspace plots with person-coloured commit nodes (FNV-1a hue, accent for "me"), ref chips, and a lane gutter; dormant/empty plots omitted with an enable-gamification empty state

## 4. Run modes, polish, and verification

- [x] 4.1 `--status` snapshot: prints the workspace/change summary and exits (verified end-to-end against the real config)
- [x] 4.2 `--line` ambient: prints one `workspaces · open changes` line and exits (verified)
- [x] 4.3 Cross-platform/degradation: responsive single-pane fallback; crossterm is cross-platform incl. Windows. Verified the full event loop end-to-end through a real pty in both truecolor and `NO_COLOR`/`TERM=dumb` (mono/ASCII) modes, plus `TestBackend` render passes at narrow/degenerate sizes (8×3, 40×12) across all five screens — exits cleanly, no panics. (A human aesthetic check on a remote terminal is the only non-automatable remainder.)
- [x] 4.4 Read-only invariant: every `AppService` call the TUI makes is a read; no workspace writes anywhere in the TUI path
- [x] 4.5 README (`crates/specforge-tui/README.md`): the three run modes, full keymap, screen tour, terminal-capability degradation, and the same-machine-shared / remote-isolated state note (with tmux/prompt `--line` examples)
