## 1. Shared config-file helper (openspec-core)

- [ ] 1.1 Add `crates/openspec-core/src/config_file.rs` and declare it with `pub mod config_file` in `lib.rs`. The module provides `write_atomic(path, bytes) -> io::Result<()>` as a thin wrapper over a private `write_atomic_with(path, bytes, before_persist)`, a private `staging_dir(path) -> &Path` (the parent, or `"."` when the parent is empty), and `read_if_present(path) -> io::Result<Option<Vec<u8>>>`. The recipe is `create_dir_all`, then `NamedTempFile::new_in`, `write_all`, `sync_all`, `persist` with its error mapped to `io::Error`, and a best-effort directory fsync (design D1).
- [ ] 1.2 Move `preserve_corrupt_config` from `crates/openspec-app/src/service.rs` into `config_file.rs` as `pub fn preserve_corrupt_config(path) -> Option<PathBuf>`. It returns the backup path when it moved the file, and keeps the `.corrupt-<n>` naming and the nanosecond fallback. Its doc comment names all four stores that use it, not only `workspaces.json` and `review-progress.json`. Move its two tests (`preserve_corrupt_config_moves_file_aside_without_losing_it`, `preserve_corrupt_config_is_a_noop_when_file_absent`) with it and assert the returned path in both.
- [ ] 1.3 Unit-test `config_file.rs`:
  - Writing a new file into a missing nested parent creates the parent and the exact bytes.
  - Overwriting a longer file with a shorter payload leaves exactly the new bytes.
  - A failure injected through `before_persist` returns `Err`, leaves the previous file byte-identical, and leaves no other entry in the directory.
  - Persisting onto an existing directory returns `Err` and leaves no staged file behind.
  - `staging_dir("a/b.json") == "a"` and `staging_dir("b.json") == "."`.
  - `read_if_present` returns `None` for a missing file, the bytes for a present one, and `Err` for a directory path.
  - `preserve_corrupt_config` skips an existing `.corrupt-0` and returns `.corrupt-1`.
- [ ] 1.4 Switch `WorkspaceRegistry::save` in `crates/openspec-core/src/registry.rs` to `config_file::write_atomic`, deleting its inline recipe. Run the existing registry tests unmodified to confirm the semantics are unchanged (`workspace-registry`: *Lossless Registry Migration and Data Preservation*).

## 2. Presentation store (openspec-core)

- [ ] 2.1 In `crates/openspec-core/src/presentation.rs`, make `WorkspacePresentationStore::save` serialize as it does now and call `config_file::write_atomic`. Make `load` use `read_if_present` with `serde_json::from_slice`: a missing file gives an empty store, a parse failure (non-UTF-8 included) gives `InvalidData`, and other read errors pass through (`workspace-registry`: *Presentation Store Saves Atomically and Survives Damage*). Update the test helper `wedge_saves`' doc comment, which says a directory can never be overwritten by `fs::write`, to say the rename refuses it.
- [ ] 2.2 Add `WorkspacePresentationStore::refusing_saves(config_path, reason: String)`. It returns an empty store whose `save` returns an `io::Error` carrying `reason` and never touches the file system. `mutate_and_save`'s existing rollback then keeps the in-memory map empty.
- [ ] 2.3 Test in `presentation.rs`, or in `crates/openspec-core/tests/presentation.rs`:
  - A store built with `refusing_saves` over an existing regular file fails `set`, `set_disabled` and `remove`, leaves the file byte-identical, and leaves `get` empty afterwards.
  - Non-UTF-8 bytes load as `InvalidData`.
  - A save after a failure injected through the helper's seam (or onto a directory path) keeps the previous file intact and rolls back the entry.

## 3. Settings store (openspec-app)

- [ ] 3.1 In `crates/openspec-app/src/settings.rs`, add `write: Mutex<()>` to `SettingsStore` and a private `update(&self, f) -> io::Result<R>` with this order: take `write`, then take `settings`, keep the previous value, apply `f`, snapshot, release `settings`, `save`, and on error restore the previous value and return the error. Document the lock order (`write` before `settings`, never the reverse) on the struct (`settings-view`: *The Settings File Is Saved Atomically and in Order*).
- [ ] 3.2 Rewrite the 25 lock-mutate-drop-save setters and `update_favorite_change_ids` as calls to `update`, so that no method mutates `settings` outside it. Keep every return type and validation exactly as it is (`set_review_skip_patterns` still validates before `update`). Read-only accessors keep taking `settings` alone. Rewrite `update_favorite_change_ids`' doc comment, which says it saves under the data lock, to describe the write lock.
- [ ] 3.3 Make `SettingsStore::save` serialize with `serde_json::to_vec_pretty` and call `config_file::write_atomic`. When the store is held (3.4), return an `io::Error` naming the path and the original read error ("… SpecForge will not save over it") without touching the file system.
- [ ] 3.4 Rewrite `SettingsStore::load` to classify the file with `read_if_present` and `serde_json::from_slice` into a public `SettingsLoad { Missing, Loaded, MovedAside(PathBuf), Unreadable(String) }`. A parse failure calls `preserve_corrupt_config`. A failed move, or a read error other than NotFound, holds the store. Add `pub fn load_outcome(&self) -> &SettingsLoad` (`settings-view`: *A Damaged Settings File Is Never Overwritten by Defaults*).
- [ ] 3.5 Update the doc comments on `DocumentWidth` and `PanelPosition` in `settings.rs` that describe the old silent reset, so they describe moving the file aside and starting from defaults.
- [ ] 3.6 Test in `settings.rs`'s test module. Every test must be deterministic.
  - A missing file loads as `Missing` with defaults and creates no file.
  - A truncated file loads as `MovedAside`. The backup is byte-identical, and a following `set_identity` writes a new `settings.json` that round-trips, with the backup unchanged.
  - An empty file and a whole-document type mismatch both load as `MovedAside`.
  - A file with an unrecognised `panelPosition` value in a provider block (and one with an unknown reading width) loads as `Loaded`, its other fields intact and no backup created.
  - A `settings.json` that is a directory at load time loads as `Unreadable` and is not moved. A setter returns the "will not save over it" error, not `EISDIR`.
  - A held store built over an existing regular file (an in-module constructor) refuses a setter, leaves the file byte-identical and keeps `snapshot()` at the pre-call value.
  - A save failure (path replaced by a directory after load) returns `Err` and `snapshot()` shows the previous value (rollback).
  - Eight threads each set a distinct setting concurrently. Afterwards a fresh `SettingsStore::load` equals the original store's `snapshot()`, compared through `serde_json::to_value` because `AppSettings` does not derive `PartialEq`, and every one of the eight values is present.
  - With `write` held by the test, `snapshot()` and `github_enabled()` called from another thread answer over a channel within a generous `recv_timeout`. The bound only matters if the code is broken.

## 4. Shared callers and bootstrap (openspec-app)

- [ ] 4.1 In `crates/openspec-app/src/review_progress.rs`, replace the body of `write` with `config_file::write_atomic(&self.path, &raw)`, and make `read_for_write` call `openspec_core::config_file::preserve_corrupt_config`. Update `write`'s doc comment to point at the shared helper, and the `tempfile` comment in `crates/openspec-app/Cargo.toml`, which names the review-progress store's `NamedTempFile` use; after this only the tests use the crate there, and it stays in `[dependencies]` as before. Run the existing review-progress tests unmodified (`pull-request-viewer`: review progress is written atomically).
- [ ] 4.2 In `AppService::bootstrap` (`crates/openspec-app/src/service.rs`), switch the registry fallback to the moved `preserve_corrupt_config`. After `SettingsStore::load`, `eprintln!` for `MovedAside` (naming the backup) and `Unreadable` (naming the reason), worded like the registry message.
- [ ] 4.3 In the same function, replace the presentation `.unwrap_or_else(|_| new(..))` with the match from design D4: `InvalidData` is moved aside and logged, then the store is `new` (or `refusing_saves` if the move failed), and any other error is logged and becomes `refusing_saves` (`workspace-registry`: *Presentation Store Saves Atomically and Survives Damage*).
- [ ] 4.4 Test in `service.rs`:
  - Bootstrapping a config directory whose `settings.json` is truncated leaves `settings.json.corrupt-0` byte-identical to the original, and `svc.settings.load_outcome()` is `MovedAside`.
  - Bootstrapping with an invalid `presentation.json` preserves it as `presentation.json.corrupt-0`, starts with no entries, and a following `set_workspace_presentation` writes a new file while the backup stays unchanged.
  - Bootstrapping where `presentation.json` is a directory leaves it in place, and `set_workspace_presentation` returns the refusal error that names the original read error. Assert the message, not just `Err`: the old `new(..)` fallback also fails here, with `EISDIR` from the write.
- [ ] 4.5 Re-run the existing skip-patterns save-failure test in `service.rs` (the one that replaces `settings.json` with a directory and expects "the skip patterns could not be saved") and confirm it still passes on the atomic path.

## 5. Verification

- [ ] 5.1 `cargo fmt --all -- --check`
- [ ] 5.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 5.3 `bun install && bun run build` once in the fresh worktree (so `dist/` exists), then `cargo test`
- [ ] 5.4 `bun run lint` (no frontend change is expected; this confirms the tree is still clean)
- [ ] 5.5 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Every mutant on a changed line in `openspec-core` and `openspec-app` must be caught, or excluded in `.cargo/mutants.toml` with a written reason (the best-effort directory fsync is the expected candidate).
- [ ] 5.6 Smoke-test the bootstrap paths with an isolated config directory: a `specforge-serve` debug build with a temporary `APP_IDENTIFIER`, never the user's real config. Copy the binary out and revert the identifier before committing. Write a truncated `settings.json` and an invalid `presentation.json`, start the server, and confirm the stderr lines and the `.corrupt-0` backups. Flip a settings switch through `POST /api/invoke`, then confirm that `settings.json` is valid JSON and holds the change while the backups are unchanged.
