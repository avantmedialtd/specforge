## Context

SpecForge is split into a headless core (`openspec-core`, no Tauri dependency) and a Tauri shell (`crates/specforge`) that wraps it with commands, events, a tray, notifications, settings, and a React frontend. The core already owns everything a frontend renders: parsing, the workspace cache, the filesystem watcher (which broadcasts `CacheEvent` over a tokio channel), git, and the math behind the dashboard, seasons/battle-pass, commit graph (including layout), and garden.

Two things a terminal frontend needs are, however, **not** in the core today — they live in the Tauri shell:

1. **The settings store** (`crates/specforge/src/settings.rs`) — `AppSettings`, `SeasonState`, the file-backed `SettingsStore`, identity seeding, and the season-recap bookmark. It has no Tauri dependency; it is in the shell only by where it grew.
2. **The dashboard *assembly*** — `get_dashboard` in `commands.rs` is ~270 lines stitching core primitives (`compute_dashboard`, `compute_progress`, `compute_leaderboard`, `season_baseline`, `compute_season`, `season_recap`, treatment unlocking) together with the activity log and settings into one `DashboardData`. It sits behind `#[tauri::command]` + Tauri `State`, so it is currently unreachable from `cargo test` and uncallable by any other frontend. First-launch backfill/seeding (`lib.rs`) is in the same boat.

The desktop app being the only frontend has kept this coupling invisible. A second frontend forces the seam — which is also a latent improvement: the most intricate, regression-prone logic in the app (the pacing/assembly a recent release had to fix) becomes testable.

## Goals / Non-Goals

**Goals:**
- A terminal-native SpecForge that browses OpenSpec workspaces, reads artifact markdown, and shows the gamified dashboard + season ladder, with live updates.
- Reuse 100% of the data layer; the TUI owns only presentation.
- Extract a shared headless `openspec-app` so both frontends compute identically and the assembly is unit-testable. The extraction must keep the desktop app green at every step and ship its value independent of the TUI.
- Run over SSH and in narrow / low-color terminals without losing legibility.
- One binary, three faces: full TUI, `--status` snapshot, `--line` ambient.

**Non-Goals:**
- Writing back to workspaces (checkbox toggling, editing) — read-only v1; `self_write` is prepped for a v2.
- A client/server "remote view of my laptop's session" — over SSH the TUI is its own SpecForge instance on the remote box, not a remote control of the desktop app.
- Replicating desktop tray / notification / dock / menu inside the terminal — those are OS-shell concerns and stay shell-only.
- Mouse support and inline images.

## Decisions

### D1. A new headless `openspec-app` crate between core and the frontends

Layering: `openspec-core` (primitives) → `openspec-app` (the stateful "brain": settings, dashboard assembly, backfill/seeding, watcher lifecycle, config-dir resolution) → `specforge` (Tauri + React) and `specforge-tui` (ratatui), each a thin frontend.

`AppService` is a facade owning the state handles (`Arc<Mutex<WorkspaceRegistry>>`, `Arc<SettingsStore>`, `Arc<Mutex<WorkspacePresentationStore>>`, `Arc<ActivityLog>`, `WatcherManager`). Its methods are the current command *bodies* with the Tauri annotations and `State` extractors stripped off. Each `#[tauri::command]` becomes a one-line delegate; the TUI calls the same methods in-process.

- *Rejected — lift into `openspec-core` directly:* works, but mixes "pure primitives" with higher-level stateful orchestration in one crate. A separate crate draws the seam exactly where the duplication risk is.
- *Rejected — duplicate the assembly in the TUI:* two copies of a 270-line stitch drift apart; the pacing bug would have to be fixed twice.

### D2. One config-dir resolver, in `openspec-app`, used by both frontends

The shell resolves the app-data directory via Tauri's `app.path().app_config_dir()` (derived from the bundle id `com.avantmedia.specforge`). The TUI has no Tauri. The resolver moves into `openspec-app` and the shell switches to it, so there is a single source of truth. This closes a silent trap: `directories::ProjectDirs` matches Tauri's path on macOS but **not on Linux** (`~/.config/com.avantmedia.specforge` vs `~/.config/specforge`), which would otherwise make the TUI read a different, empty registry beside the real one.

### D3. ratatui + crossterm, immediate mode, hand-rolled Elm/TEA loop

The TUI is render-heavy in a specific way: half is mundane (tree = list, detail = scrollable text) and half is bespoke custom cell-drawing that is the product's identity (commit-graph rail, contribution heatmap, garden, 30-tier battle-pass ladder). Immediate-mode `ratatui` gives the cell-level control the bespoke half needs and consumes the already-computed graph layout (`LaidOutCommit` / `EdgeSegment`) directly; `crossterm` is cross-platform (incl. Windows/WSL, which SpecForge supports) and integrates with tokio via its `EventStream`.

Structure: `#[tokio::main]`; `tokio::select!` over the crossterm `EventStream`, `AppService::subscribe()` (the `CacheEvent` broadcast), and a slow tick. Each event becomes a `Msg`; one `update(&mut Model, Msg)` mutates state; `view(&Model)` redraws. ratatui diffs the frame and writes only changed cells, so redraw-on-event is cheap (no busy loop). Stateful widgets (`ListState`, `ScrollbarState`) and `tui-tree-widget` handle scroll/selection so they are not re-implemented. `tui-realm` (a component/focus layer atop ratatui) is held in reserve if manual focus bookkeeping grows; it is additive and need not be adopted up front.

- *Rejected — retained frameworks (`cursive`, `iocraft`):* they win the mundane half but make custom cell-drawing the hard path and cannot cleanly consume the core's computed graph layout; `cursive` also runs its own event loop, an impedance mismatch with the tokio/broadcast design.

### D4. Honor the watcher's emit-before-broadcast ordering — re-read, never cache

`WatcherManager::emit` refreshes the aggregated view synchronously *before* subscribers wake. On a `CacheEvent` the TUI re-reads the manager (`workspace_views()`, `changes_for()`) rather than maintaining a parallel cache — the same discipline as the React `useWorkspaces` hook, minus the network. The async dashboard scan (which shells out to git) runs on a `spawn_blocking` task and posts `Msg::DataReady(DashboardData)` back to the loop, so rendering never blocks.

### D5. Presentation: one glyph language, meaning never in color alone

A fixed glyph vocabulary is shared across panels (disclosure `▾`/`▸`, change status `○`/`◐`/`●`, intensity ramp `·▫▪▓█`, locked/unlocked `▒`/`✓`). Intensity and rarity are encoded in the *glyph* as well as the color, so the UI degrades cleanly: truecolor → 256 → 16 → monochrome. A `theme` module maps `PaletteColor` onto an ANSI ladder; truecolor (which often does not survive an SSH hop) is never assumed. Emoji are double-width and inconsistent across terminals, so they are gated behind a capability check with ASCII fallbacks (e.g. streak `^12`).

Screens are modal (Browse / Dashboard / Season), switched with `1`/`2`/`3` and `Esc` — numeric switches keep the letter keys free for vim motion and avoid the `g`/`gg` collision. Browse is a two-pane master-detail with a `Tab` focus ring; below a width threshold it collapses to a single pane toggled with `Tab`. The markdown pane parses artifacts with `pulldown-cmark` and maps events to styled ratatui lines (headings, lists, code blocks, task checkboxes `☐`/`☑`, tables; alt-text for images). The four signature widgets — heatmap (`DashboardData.activity`), season ladder (scroll region, position pinned), graph rail (`EdgeSegment` → box chars), garden (`WorkspaceGarden`) — are custom `Widget` impls.

### D6. Strangler-fig migration; the desktop stays green and gains value first

The extraction lands incrementally, before any TUI code exists:

1. Create `openspec-app`; move `settings.rs` verbatim (already Tauri-free) — a pure move, shell imports it.
2. Add `openspec_app::config_dir()`; the shell switches to it.
3. `AppService::bootstrap` absorbs the `lib.rs` setup + backfill + seeding; the shell's `setup()` calls it.
4. Extract the `get_dashboard` body into `AppService::dashboard`; the command delegates. **Land the entry-baseline regression test that was previously impossible.**
5. Repeat mechanically for garden / graph / artifact / register / presentation / settings setters.
6. Only then start `specforge-tui` on top of the finished `AppService`.

## Risks / Trade-offs

- **Touching a working desktop app.** Mitigated by the strangler-fig: each step is a mechanical move that keeps the build green, and steps 1–5 deliver value (testability) even if the TUI is never built.
- **Shared state on one machine.** When the TUI and desktop app run on the same box they resolve the same config dir and co-write `activity.json` (already an accepted condition for concurrent dev instances). The real edge is the season-recap bookmark, which advances on read; two readers across a rollover could double-advance. Harden the advance to be idempotent (benefits the desktop too). Over SSH the stores are on different machines, so this edge cannot occur.
- **Terminal markdown is the one genuinely new subsystem.** `pulldown-cmark` → styled lines is the reliable path; complex tables and deeply nested formatting are the long tail and may degrade to plain text in v1.
- **The 30-tier ladder is tall.** It is the one widget whose natural shape fights a TTY; handled as a pinned-position scroll region rather than showing all tiers at once.
- **`panic = "abort"` profile.** A panic must restore the terminal (leave raw mode, show the cursor, leave the alternate screen) via a hook installed before entering raw mode, or a crash leaves a wrecked terminal. Non-optional.
- **Scope creep toward write-back.** `self_write` makes checkbox toggling tempting; explicitly deferred to keep v1 read-only and the watcher-echo handling simple.
