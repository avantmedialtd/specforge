# Ship the TUI as Downloadable Release Binaries

## Why

The `specforge-tui` terminal frontend is implemented, built, and tested in CI — but it is **not downloadable**. The release pipeline only ever runs `bun tauri build` (scoped to the `specforge` GUI crate, which has no dependency on the TUI), bundles the GUI into `.dmg` / `.deb` / `.AppImage` / NSIS `.exe`, and uploads only those bundles. The TUI binary is never compiled during a release, never bundled as a Tauri sidecar (`tauri.conf.json` has no `externalBin`), and never matched by any upload glob. A user pulling the next release gets the menu-bar GUI and nothing they can run in a terminal. To make the TUI obtainable, the release must explicitly build and attach it.

## What Changes

- The release pipeline **builds a standalone `specforge-tui` executable for each platform** as a first-class release asset, alongside the existing GUI bundles:
  - **Linux**: `cargo build --release -p specforge-tui` on the existing `ubuntu-latest` job (the TUI is pure terminal — no webkit/gtk).
  - **Windows**: `cargo xwin build --release -p specforge-tui --target x86_64-pc-windows-msvc` on the existing cross-compile job (`cargo-xwin` is already installed there).
  - **macOS**: build `x86_64-apple-darwin` + `aarch64-apple-darwin` and `lipo -create` a universal binary on the existing `macos-latest` job (both targets are already installed for the universal GUI).
- Each binary is **packaged as a compressed archive** — `.tar.gz` for macOS/Linux, `.zip` for Windows — so the executable bit survives download/extraction and users skip `chmod +x`. The archive carries the tag version and platform/arch in its name (e.g. `specforge-tui_0.2.0_macos-universal.tar.gz`).
- The archives are **added to each build job's `upload-artifact` paths** (keeping `if-no-files-found: error`), so they ride the existing `merge-multiple: true` → `files: dist/**/*` flow into the published GitHub Release with no change to the `release` job.
- The `/release` notes template (Downloads footer) gains a **TUI section**, including the macOS **Gatekeeper quarantine** workaround (`xattr -dr com.apple.quarantine specforge-tui`) — a terminal binary has no right-click ▸ Open dialog, so this must be documented or the macOS download is dead on arrival.

This is purely additive — no change to the GUI bundles, the trigger, the version stamping, or the publish job. It reuses the three existing build jobs (no new runners) and the existing version-stamp step (the TUI inherits `[workspace.package].version`, so its version already matches the tag for free).

## Capabilities

### Modified Capabilities
- `release-pipeline`: in addition to the per-platform GUI bundles and the Windows portable `.exe`, the pipeline now emits a standalone `specforge-tui` CLI binary per platform — universal on macOS, cross-compiled for Windows via `cargo-xwin`, native on Linux — packaged as a compressed archive, unsigned, version-matched to the tag, with the macOS quarantine caveat documented in the release notes.

## Impact

- **`.github/workflows/release.yml`**: one added build step in each of the three build jobs (`cargo`/`cargo xwin` build of `-p specforge-tui`, archive packaging, macOS `lipo`), and the new archive paths added to each job's `upload-artifact` `path:`. The `release` publication job is **unchanged** — `files: dist/**/*` already uploads whatever the build jobs hand it.
- **`/release` command + notes template** (`release-command` skill): the Downloads footer gains TUI install lines and the macOS quarantine note. No spec change to `release-command` — per the `release-pipeline` precedent (the WebView2 "Prerequisite Documented" requirement lives in `release-pipeline`), the gatekeeper-documentation requirement is captured here too.
- **No application code changes.** No Rust source, no `tauri.conf.json` `externalBin`, no new crate. The TUI is built straight from the existing workspace; the GUI crate and its bundles are untouched.
- **CI cost**: a `cargo build -p specforge-tui` added to each release job — cheap, because `tauri build` has already compiled the shared `openspec-app`/`openspec-core` graph the TUI reuses, and the per-job Rust cache is warm.
- **Risk**: low and well-bounded. The chief unknown is the Windows `cargo-xwin` build of a non-Tauri binary (the existing job only cross-compiles through `tauri build --runner cargo-xwin`); a plain `cargo xwin build -p specforge-tui` is the standard cargo-xwin invocation and the `if-no-files-found: error` guard fails the job loudly if it produces nothing.
