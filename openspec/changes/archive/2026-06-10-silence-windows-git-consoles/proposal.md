# Silence Windows Git Console Flashes

## Why

On Windows, the packaged SpecForge build runs in the GUI subsystem (`windows_subsystem = "windows"` in `crates/specforge/src/main.rs`), so the process has no console. Every time it spawns a console-subsystem child — `git.exe` for local repositories, `wsl.exe -d <distro> git …` for WSL-hosted ones — Windows allocates a fresh console for the child, and a conhost window flashes on screen.

WSL workspaces turned this from an occasional annoyance into a steady strobe: the polling watcher re-scans every 10 seconds and each re-parse runs several git calls, so a registered WSL workspace flashes console windows continuously while the app idles. The plain `git.exe` path has the same defect (it predates WSL support); it just fires less often.

Dev builds never show this because `bun tauri dev` runs in the console subsystem from a terminal, so children inherit the existing console — which is why the defect shipped unnoticed.

## What Changes

Apply the `CREATE_NO_WINDOW` (0x08000000) process-creation flag to every git child process spawned on Windows. All production spawns funnel through the single `git_command()` chokepoint in `crates/openspec-core/src/git.rs` (both the `wsl.exe` arm and the plain `git` arm), so the fix is a few lines in one function via `std::os::windows::process::CommandExt::creation_flags`. The child still receives a console object — console APIs keep working — but no window is ever created. All call sites use `.output()` with piped stdio, so nothing reads or writes that invisible console.

No new dependencies (the flag is a one-line constant; `CommandExt` is std), no behaviour change off Windows (the flag is `#[cfg(target_os = "windows")]`-gated and compiles away), no API or IPC changes, no frontend involvement.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `wsl-workspaces`: add a requirement that git child processes on Windows (both direct `git.exe` invocations and `wsl.exe`-routed ones) are spawned without creating visible console windows.

## Impact

- `crates/openspec-core/src/git.rs` — `git_command()` gains a Windows-gated `creation_flags(CREATE_NO_WINDOW)` applied to both command arms.
- `openspec/specs/wsl-workspaces/spec.md` — one added requirement.
- No other code paths spawn OS processes (audited: remaining `Command::new` sites are `#[cfg(test)]`; the Tauri shell and its plugins spawn no console children).
- Like the rest of the WSL backend, end-to-end verification (no flashes while a WSL workspace polls) needs a real Windows box; `cargo test` is unaffected since the flag only suppresses window creation.
