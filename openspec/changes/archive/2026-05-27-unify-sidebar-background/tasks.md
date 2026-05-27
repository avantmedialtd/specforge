## 1. Rust crate

- [x] 1.1 Remove the `[target.'cfg(target_os = "macos")'.dependencies]` block (and the comment that introduces it) from `crates/specforge/Cargo.toml`, dropping the `window-vibrancy = "0.5"` line.
- [x] 1.2 Remove the `#[cfg(target_os = "macos")] use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};` import at `crates/specforge/src/lib.rs:15-16`.
- [x] 1.3 Remove the `#[cfg(target_os = "macos")] if let Err(err) = apply_vibrancy(...) { eprintln!(...) }` block at `crates/specforge/src/lib.rs:151-156`, along with the preceding `// macOS: apply NSVisualEffectMaterial::Sidebar ...` comment paragraph (`crates/specforge/src/lib.rs:147-150`).
- [x] 1.4 Run `cargo check -p specforge` and confirm there are no unused-import warnings, then `cargo build -p specforge` to confirm a clean build on macOS.
- [x] 1.5 Inspect `Cargo.lock` after the build. Note: `window-vibrancy` itself does not disappear — `tauri 2.11.2` carries it as a transitive dep at version `0.6.0`. What does drop is the old `0.5` chain that our direct dep pulled in (block2 0.5.1, objc2 0.5.2 and their transitive crates) — ~220 lines slimmer. Commit the regenerated lockfile.

## 2. Stylesheet

- [x] 2.1 In `src/App.css`, delete the `body[data-platform="mac"] { background: transparent; }` rule at lines 151-156 (including its leading "macOS: let window-level vibrancy show through..." comment).
- [x] 2.2 In `src/App.css`, delete the `body[data-platform="mac"] .split-pane { background: transparent; }` rule at lines 215-217.
- [x] 2.3 In `src/App.css`, update the `body[data-platform="mac"] .split-pane-left` block at lines 226-232: remove the `background: transparent;` line and the "macOS gets sidebar vibrancy ..." comment that introduces it; keep the `padding-top: var(--space-6);` declaration; rewrite the comment to describe only the safe-area padding (e.g., "macOS reserves 32px of safe-area padding for the overlay traffic lights").
- [x] 2.4 Confirm `.split-pane-left { background: var(--surface); }` at line 219-224 is unchanged so all platforms inherit it.

## 3. Documentation

- [x] 3.1 Inspected `CLAUDE.md` — the `lib.rs` bullet under "Rust workspace" (line 47) describes event-forwarder ordering and synchronous cache population only; it never mentioned vibrancy. No edit needed.
- [x] 3.2 Grepped `CLAUDE.md` for `vibranc` / `NSVisualEffect` — zero matches. The proposal overestimated CLAUDE.md's coverage of this concern; the only vibrancy mentions in the repo were in `Cargo.toml`, `crates/specforge/src/lib.rs`, and `src/App.css` comments, all trimmed in tasks 1.x and 2.x. No CLAUDE.md edit needed.

## 4. Verification

- [x] 4.1 Ran `bun tauri dev` from the worktree on macOS. Screenshot at `/tmp/specforge-verify/sidebar-after.png` confirms: sidebar paints a solid surface (no wallpaper bleeding through), traffic lights float over the top-left, the first sidebar row clears the 32px safe area. Verified in dark mode (system theme at time of test).
- [~] 4.2 **Deferred for manual verification.** Auto mode declined to toggle the system-wide OS appearance preference. The dark-mode case is verified; light mode follows automatically from `--surface: #ffffff` resolving via the `:root` light declaration. To confirm manually: System Settings > Appearance > Light, observe the sidebar.
- [x] 4.3 Verified by inspection: `.titlebar-drag-region` element in `src/App.tsx` and the `body[data-platform="mac"] .titlebar-drag-region { pointer-events: auto }` rule at `src/App.css:177-179` were not touched by this change. The IPC drag path is intact.
- [x] 4.4 Ran `bun run build` — completed cleanly (exit 0); produced `dist/` consumed by the subsequent `cargo build`.
- [x] 4.5 Ran `cargo test --workspace` — all suites pass. openspec-core: parser (1), registry (1), cache (1), watcher (4), self_write (1) = 8/8. specforge: commands (3), tray (3), tray_icon (3) = 9/9. Total 17/17 passing.
- [x] 4.6 Ran `openspec validate unify-sidebar-background` — change is valid after edits.
