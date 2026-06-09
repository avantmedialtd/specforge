## 1. Foundations — pure WSL module + canonicalisation hygiene (cross-platform, fully unit-testable)

- [x] 1.1 Add `dunce` to `crates/openspec-core/Cargo.toml`.
- [x] 1.2 Add a shared `canonicalize` helper (backed by `dunce::canonicalize` / `dunce::simplified`) in `openspec-core`, and route every existing raw `std::fs::canonicalize` call site through it: `registry.rs` (register/unregister/discover), `commands.rs::read_artifact`, and `git::git_common_dir`.
- [x] 1.3 Create `crates/openspec-core/src/wsl.rs` with the pure primitives: `is_wsl_path(&Path) -> bool`, `parse_wsl_path(&Path) -> Option<WslPath { distro, linux_path }>` (handles `\\wsl$\`, `\\wsl.localhost\`, and verbatim `\\?\UNC\wsl…`), `wsl_to_unc(distro, linux_path) -> PathBuf`, and `watch_strategy(&Path) -> WatchStrategy::{Native, Poll}`. Annotate the module `#[cfg_attr(not(target_os = "windows"), allow(dead_code))]`; it contains no Windows API and no process execution.
- [x] 1.4 Unit-test detection on all four forms plus a local drive-letter path, and the Linux↔UNC translation round-trip (`wsl-workspaces`: *WSL Workspace Path Detection*, *Linux and UNC Path Translation*). Tests must compile and pass on macOS/CI.
- [x] 1.5 Unit-test that two equivalent path forms (simplified UNC vs verbatim `\\?\UNC\…`) canonicalise to one representation (`workspace-registry`: *Git Repository Detection* — equivalent-forms scenario).

## 2. Watcher backend — per-workspace polling for WSL (Windows-gated)

- [x] 2.1 Introduce a `#[cfg(target_os = "windows")]` `WatcherKind` enum (`Native(Debouncer<RecommendedWatcher, FileIdMap>)` / `Poll(Debouncer<PollWatcher, FileIdMap>)`) in `watcher.rs`; on non-Windows leave `WatcherEntry` holding the native debouncer exactly as today.
- [x] 2.2 In `add_workspace`, choose the backend via `watch_strategy(&workspace.uri)`; for the `Poll` arm build the debouncer with `new_debouncer_opt::<PollWatcher, _>(…)` and `notify::Config::with_poll_interval(<configured>)`. Keep the mpsc bridge, `Weak<Inner>` task, and `openspec/changes/` filter identical across both arms (`wsl-workspaces`: *Polling Watcher for WSL Workspaces*).
- [x] 2.3 Thread a poll-interval `Duration` (default 10s) into `WatcherManager` alongside the existing `debounce` field, so the shell can configure it.
- [x] 2.4 Unit-test `watch_strategy` selection (Poll iff `is_wsl_path`, else Native) as pure logic; the behavioural "poll actually fires over 9P" check is deferred to the spike (group 5).

## 3. Git routed through `wsl.exe` (Windows-gated)

- [x] 3.1 Centralise git `Command` construction in `git.rs` so a WSL workspace yields `wsl.exe -d <distro> git -C <linux_path> …`, translating path arguments (e.g. `-C <cwd>`) to Linux form. Gate the WSL branch `#[cfg(target_os = "windows")]`; non-Windows constructs only the native `git -C` command.
- [x] 3.2 Translate path outputs back to UNC via `wsl_to_unc`: worktree porcelain paths and `--git-common-dir`, so the registry/cache store consistent Windows-side paths (`wsl-workspaces`: *Git Operations Routed Through the WSL Distribution* — worktree-list and identity scenarios).
- [x] 3.3 Preserve graceful degradation: when `wsl.exe` is missing or the distro is unreachable, the git call returns `None` and the workspace stays a flat workspace (`wsl-workspaces`: missing-`wsl.exe` scenario; `workspace-registry`: `git`-missing scenario).
- [x] 3.4 Unit-test argv construction and Linux→UNC output translation with synthetic porcelain (build the `Command` / map strings without executing) so the logic is verified on macOS/CI.

## 4. Configurable poll interval — Windows-only setting surface

- [ ] 4.1 Add a persisted poll-interval field (default 10s) to `AppSettings` in the `specforge` shell, gated/surfaced on Windows, and plumb it into `WatcherManager` at startup and when it changes (`wsl-workspaces`: *Configurable Poll Interval*).
- [ ] 4.2 Surface the interval control in the Settings view on Windows only; if the value crosses the IPC boundary, mirror the Rust type in `src/types.ts` (no codegen — keep both sides matched).
- [ ] 4.3 Confirm the macOS and Linux builds compile with the WSL backend *and* the setting absent (`wsl-workspaces`: *Windows-Scoped WSL Backend* — non-Windows-excludes-backend scenario).

## 5. Validation spike — real Windows + WSL2 box (the four behavioural checks)

- [ ] 5.1 On a Windows host with WSL2, register a workspace at `\\wsl.localhost\<distro>\…` and confirm the `PollWatcher` reflects an edit made *inside* the distro within one poll interval.
- [ ] 5.2 Confirm `wsl.exe -d <distro> git -C <linux> worktree list --porcelain` returns the expected porcelain and that the Linux→UNC translation round-trips against it.
- [ ] 5.3 Register two worktrees of the same WSL repository and confirm they resolve to one stable `RepoId` (single badge count, one aggregated group).
- [ ] 5.4 Confirm end-to-end edit→UI latency is acceptable at the default 10s and that lowering the setting shortens it as expected.
- [ ] 5.5 Record findings; if poll visibility over 9P fails, fall back to an explicit manual-refresh affordance rather than shipping a silently-stale dashboard (per design risk).

## 6. Verification & wrap-up

- [ ] 6.1 `cargo test` green on macOS/CI (pure detection, translation, `watch_strategy`, canonicalisation, git argv/output tests).
- [ ] 6.2 Build the Windows target (cross-compile or CI) and confirm the WSL backend compiles in; confirm macOS/Linux targets compile with it gated out.
- [ ] 6.3 Add a short note (CLAUDE.md or README) documenting that WSL support is Windows-only, poll-based, and routes git through `wsl.exe`, with the 10s configurable interval.
