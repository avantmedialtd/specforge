# Tasks

## 1. Refresh aggregated views on register

- [x] 1.1 In `crates/specforge/src/commands.rs`, inside `register_workspace`, call `watcher.aggregate_and_emit()` after the existing per-folder `add_workspace` loop and the `sync_repos()` call, before the `repo_id` lookup block — so `last_views` is rebuilt while we still hold the relevant `State<'_, _>` handles.
- [x] 1.2 Confirm the registry lock acquired earlier in the function has been released before the new call. (The existing code already drops the guard at the end of its block; just verify no refactor accidentally extended the scope.)

## 2. Refresh aggregated views on unregister

- [x] 2.1 In `crates/specforge/src/commands.rs`, inside `unregister_workspace`, call `watcher.aggregate_and_emit()` once after the loop that invokes `watcher.remove_workspace(p)` for each removed path and after `watcher.sync_repos()`. A single call after the loop covers the cascade case (one user-registered path plus any discovered worktrees dropped with it).
- [x] 2.2 Verify the call is reached even when `removed` is empty — the recomputation is idempotent in that case and adds no harm. (If we wanted to skip it when nothing changed, gate on `any_removed`; either choice is fine, default to unconditional for simplicity.)

## 3. Verify the fix end-to-end

- [x] 3.1 `cargo test` passes at the workspace root.
- [x] 3.2 `bun run build` succeeds (the bug is Rust-only but the build verifies nothing on the frontend was knocked over).
- [x] 3.3 Manual: `bun tauri dev`. Register a workspace from Settings → confirm it appears in the tree pane immediately, without needing a file change or a restart. Remove it → confirm it disappears in the same update.
- [x] 3.4 Manual cascade: register a workspace whose repository has at least one other worktree (so auto-discovery kicks in), then unregister the user-registered one → confirm the entire repo group disappears from the tree in one update.

## 4. (Optional) Lock down the contract with a Rust integration test

- [x] 4.1 ~~Consider adding an integration test in `crates/specforge/tests/` (or the equivalent) that drives the `register_workspace` / `unregister_workspace` IPC handlers programmatically against a `WatcherManager` and asserts `watcher.workspace_views()` shape after each call.~~ **Skipped.** The `specforge` crate has no `tests/` integration directory (only inline `#[cfg(test)] mod tests` in `lib.rs`/`commands.rs`), and the existing `commands::tests` exercise only the pure helpers (e.g. `presentation_keys_to_drop`). Driving the `#[tauri::command]` handlers programmatically would require constructing a `tauri::test::mock_app` setup that doesn't exist anywhere else in this crate — non-trivial harness work for a 2-line fix. The watcher-side mechanism is already covered by `crates/openspec-core/tests/watcher.rs` (`add_workspace_populates_cache`, `remove_workspace_clears_cache_and_watcher`). The spec scenarios in `specs/workspace-registry/spec.md` plus the manual checks in §3 are the load-bearing verification.
