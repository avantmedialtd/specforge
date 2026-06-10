# Tasks: Silence Windows Git Console Flashes

## 1. Suppress console windows at the spawn chokepoint

- [x] 1.1 In `crates/openspec-core/src/git.rs`, apply `creation_flags(CREATE_NO_WINDOW)` (`0x0800_0000`, via `std::os::windows::process::CommandExt`, behind `#[cfg(target_os = "windows")]`) to the `wsl.exe` command arm of `git_command()` before its early return
- [x] 1.2 Apply the same flag to the plain `git` command arm of `git_command()` so direct `git.exe` invocations on Windows are covered too
- [x] 1.3 Document the constant and flag choice in a brief comment stating the constraint (GUI-subsystem parent + console-subsystem child ⇒ window unless suppressed), per design D1/D3

## 2. Verify

- [x] 2.1 Audit that no other production `Command::new` / spawn sites exist outside `git_command()` (the known ones in `registry.rs` and `git.rs` tail are `#[cfg(test)]`); record the result in the task notes
  - Audited: production spawns are exactly `git.rs:170` (`wsl.exe`) and `git.rs:180` (`git`), both now flagged. `git.rs:1046/1419/1503` and `registry.rs:371` are inside `#[cfg(test)]` modules. The Tauri shell and its plugins spawn no OS processes.
- [x] 2.2 Run `cargo test` (workspace) — green, no functional change expected on any platform
  - 241 tests passed, 0 failed.
- [x] 2.3 Run `cargo clippy --workspace --all-targets` and `cargo fmt --check`; confirm the Windows-gated code paths at least compile via `cargo check --target x86_64-pc-windows-msvc` if the toolchain target is available, otherwise note that CI's Windows job is the compile gate
  - clippy (workspace, all targets): clean. `cargo fmt --all --check`: clean. Target was installed locally: `cargo check` **and** `cargo clippy` for `openspec-core` on `x86_64-pc-windows-msvc` both pass — the gated `CommandExt`/`creation_flags` code compiles for real Windows.
- [x] 2.4 Note the residual: visual confirmation (no conhost flashes while a WSL workspace polls) requires a real Windows box — same residual as the original `wsl-workspaces` change
  - Residual stands: window suppression is documented Win32 behaviour (`CREATE_NO_WINDOW`) and the ecosystem-standard pairing with `wsl.exe` (VS Code Remote-WSL), but "no flash while polling" can only be eyeballed on Windows + WSL2 hardware.
