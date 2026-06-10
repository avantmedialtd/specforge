# Design: Silence Windows Git Console Flashes

## Context

SpecForge's release binary is a GUI-subsystem process on Windows (`windows_subsystem = "windows"`), so it owns no console. Win32 rules: when a console-subsystem child is spawned by a parent without a console (and without flags saying otherwise), the OS allocates a new console and shows its window. Every `git.exe` and `wsl.exe` spawn therefore flashes a conhost window in the packaged app.

All production process spawns funnel through one function — `git_command()` in `crates/openspec-core/src/git.rs` — which builds either a `wsl.exe -d <distro> git …` command (WSL-hosted anchor, Windows only) or a plain `git` command. Every call site executes via `.output()`, i.e. with stdin/stdout/stderr piped. The remaining `Command::new` occurrences in the workspace are `#[cfg(test)]`; the Tauri shell and its plugins (notification, autostart, window-state, dialog) spawn no console children.

The WSL polling watcher makes this acute: a 10-second re-scan cadence times several git calls per re-parse means near-continuous window flashing while a WSL workspace is registered.

## Goals / Non-Goals

**Goals:**
- No visible console window for any git child process spawned by SpecForge on Windows — both direct `git.exe` and `wsl.exe`-routed invocations.
- Zero behaviour change off Windows and zero change to command output, exit codes, or error handling.

**Non-Goals:**
- Reducing the *number* of git calls per poll tick (separate concern; the calls should simply be invisible).
- Touching any other process-spawning mechanism (there are none in production code).
- Prerequisites for interactive git (credential prompts etc.) — all invocations are read-only porcelain and already assume non-interactive operation.

## Decisions

### D1: `CREATE_NO_WINDOW`, not `DETACHED_PROCESS` or wrappers

Apply `creation_flags(CREATE_NO_WINDOW)` (`0x0800_0000`) via `std::os::windows::process::CommandExt`:

- `CREATE_NO_WINDOW` gives the child a console *object* with no *window* — console APIs keep working, piped stdio is unaffected. This is the flag VS Code uses to drive `wsl.exe` for Remote-WSL, so the exact pairing (hidden console + `wsl.exe`) is well-trodden.
- `DETACHED_PROCESS` gives the child no console at all; some console programs (historically including `wsl.exe`) misbehave without one. Rejected.
- `cmd /c start /b` wrappers add a `cmd.exe` layer — the thing being removed. Rejected.
- `STARTUPINFO.wShowWindow = SW_HIDE` still creates the window (hidden, racy) and isn't cleanly exposed by `std::process`. Rejected.

### D2: Apply at the `git_command()` chokepoint, both arms

The WSL arm early-returns (`return cmd;`), so the flag is applied independently in both the `wsl.exe` arm and the plain `git` arm rather than restructured into a single exit point — keeping the function's existing shape and the WSL block's `#[cfg(target_os = "windows")]` gating intact. The plain arm gets its own `#[cfg(target_os = "windows")]` block.

### D3: Hardcode the constant; no new dependency

`const CREATE_NO_WINDOW: u32 = 0x0800_0000;` defined locally. `openspec-core` has no `windows-sys` dependency today and one documented Win32 constant does not justify adding it. `CommandExt` is std.

## Risks / Trade-offs

- **Cannot be verified end-to-end off Windows.** Same caveat as the rest of the WSL backend: the "no flash while polling" behaviour needs a real Windows box. Mitigation: the mechanism is documented Win32 behaviour and ecosystem-standard; `cargo test` confirms no functional regression (the flag affects window creation only — tests run from a console where children inherit it regardless).
- **Ancient WSL builds (~2018) had bugs when run without a visible console.** Out of scope: WSL detection requires `\\wsl.localhost`, which only modern WSL exposes.
- If a future git invocation ever needed an interactive console prompt, it would hang invisibly instead of flashing a window. Acceptable: every invocation is read-only porcelain with piped stdio, and an interactive prompt would already hang a GUI app today.
