## 1. Fix the macOS branch of `set_badge`

- [x] 1.1 In `crates/specforge/src/tray.rs`, change the macOS branch of `set_badge` so it always passes `Some(&str)` to `tray.set_title`, substituting `""` for the no-count case. The non-macOS branch is untouched.
- [x] 1.2 Confirm the `count.filter(|&n| n > 0)` short-circuit still routes `Some(0)` and `None` through the empty-title path (preserves the existing "0 ≡ absent" semantics).

## 2. Regression test

- [x] 2.1 Add a test in `crates/specforge/src/tray.rs` (`#[cfg(test)] mod tests`) or a new `crates/specforge/tests/tray_badge.rs` that pins the macOS title-string contract: count `Some(0)`/`None` → empty string; count `Some(n)` (n ≥ 1) → `n.to_string()`. The test SHOULD NOT instantiate a real `TrayIcon` — either extract the title-string logic into a pure helper (preferred, see design Decision 3) or use a thin trait-based recorder.
- [x] 2.2 Run `cargo test -p specforge` and confirm the new test passes alongside the existing suite.

## 3. Verify in the running app

- [ ] 3.1 Start the dev app (`bun tauri dev`). With at least one registered workspace, archive its single active change and confirm the menu-bar badge text disappears (status item collapses to icon-only width).
- [ ] 3.2 Repeat with a higher non-zero count (e.g. 3 → 2 → 1 → 0) to confirm the non-zero transitions still render the correct digit and only the final 1 → 0 step collapses the title.

## 4. Workspace housekeeping

- [x] 4.1 Run `cargo fmt` and `cargo clippy --workspace -- -D warnings` and clear any new lint output.
- [x] 4.2 Update `CLAUDE.md` only if the fix changes a documented invariant for `tray.rs` (likely not — the macOS-template-rendering note is unaffected). _No update needed; the new invariant is captured in the tray-indicator spec._
