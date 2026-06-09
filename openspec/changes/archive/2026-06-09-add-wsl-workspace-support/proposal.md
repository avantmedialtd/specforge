# Add WSL workspace support for the Windows app

## Why

A large cohort of Windows developers keep their code inside the **WSL2 Linux
filesystem** (the recommended practice — the Windows-mounted `/mnt/c` path is
slow), reaching it from Windows tools via the `\\wsl.localhost\<distro>\…` 9P
share. Today the Windows SpecForge build cannot serve them: file-change
notifications never cross the 9P boundary, so the app would register a
WSL-hosted workspace, render its initial state, and then go **permanently
deaf** — and `git`-driven features fail outright on the Linux checkout. Since
SpecForge is fundamentally a live-watching dashboard, "deaf" is the product's
heartbeat stopping, not a soft degradation. This change makes WSL-hosted
OpenSpec workspaces first-class for the Windows app.

## What Changes

- **Detect WSL-hosted workspaces** by their UNC shape (`\\wsl$\<distro>\…`,
  `\\wsl.localhost\<distro>\…`, and the verbatim `\\?\UNC\wsl…` form), and
  carry the parsed `(distro, linux_path)` through the parts of the core that
  need it.
- **Watch WSL workspaces by polling.** `notify`'s Windows backend
  (`ReadDirectoryChangesW`) receives no events over the 9P share, so the
  per-workspace watcher switches to `notify::PollWatcher` (a stat sweep on a
  **10s default interval, user-configurable**) for WSL paths. Local drive-letter
  workspaces keep the event-driven native watcher unchanged. The choice is
  **per workspace** — a user can have local and WSL workspaces watched
  simultaneously by different backends.
- **Route `git` through `wsl.exe` for WSL repos.** Git metadata
  (worktree list, common-dir, branch, commit graph) is gathered by invoking the
  **native Linux `git`** inside the distro (`wsl.exe -d <distro> git …`) rather
  than pointing Windows `git.exe` at the 9P checkout. This sidesteps the
  "dubious ownership" `safe.directory` guard and the 9P `.git` performance
  cliff. Linux paths returned by git are **translated back to UNC** so the rest
  of the app keeps using consistent Windows-side paths for filesystem reads.
- **Stabilise path identity** by routing every `canonicalize()` through a
  single `dunce`-based helper, so a WSL repo is never split into two `RepoId`s
  by verbatim-vs-simplified UNC forms (which would double-count the tray badge
  and fracture the aggregated view).
- **A documented validation spike.** The 9P-specific behaviour (poll fires,
  `wsl.exe` git porcelain, stable `RepoId`, acceptable latency) can only be
  proven on a real Windows + WSL2 machine; the change carries that as an
  explicit, scoped risk rather than an assumed fact.

This is **additive** — no behaviour changes for macOS, Linux, or
local-drive Windows workspaces.

## Capabilities

### New Capabilities

- `wsl-workspaces`: detection and Linux↔UNC translation of WSL paths, the
  per-workspace poll-watcher strategy for 9P shares, and the `wsl.exe`-routed
  git backend with output path translation. (Spec deltas deferred — this
  proposal + design capture the thinking first.)

### Modified Capabilities

- `workspace-registry`: registration/canonicalisation gains `dunce`-based path
  hygiene so UNC/verbatim forms resolve to a single stable `RepoId`. (Delta
  deferred alongside the new capability.)

## Impact

- **Crate:** all logic lands in `openspec-core` (`wsl.rs` new module,
  `watcher.rs` watcher-backend enum, `git.rs` command construction + output
  translation, the shared `canonicalize` helper) — keeping it `cargo test`-able
  without the Tauri GUI, per the project's core/shell split.
- **New dependency:** `dunce` (tiny, path-normalisation only). `notify` 6 and
  `notify-debouncer-full` 0.3 already provide `PollWatcher` /
  `new_debouncer_opt` — no version bumps.
- **Runtime dependency (WSL repos only):** `wsl.exe` present and the target
  distro reachable. When absent, git ops degrade to `None` as they do today —
  the WSL workspace still parses, polls, and reads as a *flat* workspace.
- **Platforms:** no change for macOS / Linux / local-drive Windows. The WSL
  **backend** (poll-watcher arm, `wsl.exe` git routing, poll-interval setting) is
  `#[cfg(target_os = "windows")]`-gated, so it never compiles into the macOS or
  Linux builds. The pure `wsl.rs` helpers stay cross-compiled (inert off-Windows)
  purely so their unit tests run on the macOS/CI runners.
- **Testing:** pure logic (detection, translation, watch-strategy selection,
  git argv construction, `dunce` round-trips) is unit-tested on the existing
  macOS/CI runners; the 9P-behavioural claims are validated by a one-time
  Windows spike.
