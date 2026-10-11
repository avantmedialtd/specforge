## 1. Shared config-file helpers

- [ ] 1.1 If `atomic-settings-persistence` has landed, use `openspec-core/src/config_file.rs` (`write_atomic`, `preserve_corrupt_config`) as is. Otherwise create `crates/openspec-core/src/config_file.rs` exactly as `atomic-settings-persistence` tasks 1.1-1.3 specify, with the same signatures (`write_atomic(path, bytes) -> io::Result<()>`, `preserve_corrupt_config(path) -> Option<PathBuf>`) and tests, so that change later rebases onto it with nothing to reconcile. `write_atomic` follows the `tempfile::NamedTempFile::persist` recipe in `registry.rs` (~431-444); `preserve_corrupt_config` moves from `crates/openspec-app/src/service.rs` (~222) with its two tests. Keep a single copy of each.

## 2. Activity log: append-only JSON Lines (openspec-core)

- [ ] 2.1 In `crates/openspec-core/src/activity_log.rs`, replace `ActivityLog::load(path)` and `persist()` with `ActivityLog::open(config_dir)`, which owns `activity.jsonl`, `activity.lock` and the legacy `activity.json`. Rewrite the module doc so it says that appends cost O(batch), drops the "event volume is modest" claim, and describes the lease. (`activity-log`: *Incremental, Ordered, Crash-Safe Log Persistence*)
- [ ] 2.2 Make `record_all` push and append under one hold of the events mutex. Serialise the batch as compact JSON lines into one buffer and send it with a single `write_all` to the append-mode handle, only while the lease is held. Make `record` delegate to `record_all(vec![event])`. Keep the empty batch as a no-op that writes nothing.
- [ ] 2.3 Make `reconcile_lifecycle` compute `missing_lifecycle_events` and append the result inside the same critical section, instead of locking, dropping the guard and re-locking.
- [ ] 2.4 Implement the persistence lease (design Decision 4). `open` calls `File::try_lock()` on `activity.lock` and keeps the handle while it holds the lock. A non-holder calls `try_lock()` again on each `record_all`, trims any torn tail when it acquires the lock, and persists only the batches it records after acquiring. (`activity-log`: *Single Persisting Instance*)
- [ ] 2.5 Implement load (design Decision 5) as follows. All writes belong to the holder only; a non-holder only parses.
  - Parse line by line.
  - Treat an unterminated final fragment (no trailing `\n`) that follows at least one complete parseable line as a torn tail: drop it, and have the holder truncate the file to the end of the last complete line.
  - Treat everything else that fails to parse as damage, including a terminated final line and a file with no complete parseable line. Move the file aside with `preserve_corrupt_config`, then `write_atomic` the parsed events (possibly none) to a fresh `activity.jsonl`. Never truncate in this case.
  - A non-holder parses `activity.jsonl`, or the legacy `activity.json` when there is no `activity.jsonl`, keeps every line that parses, and writes, truncates and moves nothing.
  - Flag a failed or partial append so that the next append re-checks the tail first.
- [ ] 2.6 Implement the legacy migration. Write a parseable `activity.json` array to `activity.jsonl` with `write_atomic`, then rename it to the first free name of `activity.json.migrated`, `activity.json.migrated-1`, … so an existing kept copy is never replaced (`rename` overwrites silently). If both files exist, `activity.jsonl` wins and the legacy file is renamed the same way. Move an unparseable `activity.json` aside with `preserve_corrupt_config` and start empty.
- [ ] 2.7 Add tests in `activity_log.rs`, deterministic and on temp directories only:
  - Appending a two-event batch to a populated log leaves every earlier byte unchanged and adds exactly two lines.
  - An empty batch writes nothing.
  - Events from several threads reload in the order they were recorded in memory.
  - An unterminated final fragment after good lines is dropped and truncated, earlier events survive, no `.corrupt-*` file appears, and `is_empty()` is false.
  - A terminated but unparseable final line is treated as damage: `.corrupt-0` holds the original bytes and the earlier events survive.
  - A bad line in the middle produces an `activity.jsonl.corrupt-0` holding the original bytes, and a clean file holding every parseable event plus later appends.
  - A wholly unparseable file, and a file holding only one unterminated fragment, are moved aside byte-for-byte and load empty.
  - A non-holder opening a damaged or torn file loads the parseable events and leaves the file's bytes unchanged.
  - A legacy array migrates with identical fields and order, `activity.json.migrated` exists afterwards, and later records append.
  - Both files present makes the jsonl win; with `activity.json.migrated` already present the leftover becomes `activity.json.migrated-1` and the earlier copy is byte-identical.
  - An unparseable legacy file is preserved as `.corrupt-<n>`.
  - Two `ActivityLog::open` handles on one directory: only the first appends, the second still answers `query_window` with its own event, and once the first is dropped the second's next `record_all` appends.
  - Two concurrent `reconcile_lifecycle` calls with the same lifecycles append each missing event once.

  These kill the mutants on the changed lines; do not use sleeps. (`activity-log`: *Incremental, Ordered, Crash-Safe Log Persistence*, *Single Persisting Instance*)
- [ ] 2.8 In `crates/openspec-core/src/watcher.rs` `handle_events`, move the archived-id computation (`removed_ids` and `list_archived_changes`, ~1162-1171) up to just before the batch's `record_all` (~1116-1118), after the `cache.contains` guard and before any broadcast. Append the archival `ChangeArchived` achievements (~1178-1191) to the batch's achievement `Vec`, so the batch makes a single `record_all`. Keep emitting the `CacheEvent::ChangeArchived` events per id, in their current order and position. Extend the watcher tests in `crates/openspec-core/tests/watcher.rs`: a batch that archives several changes results in one append that holds all of the archivals, and a batch for a workspace removed mid-parse records nothing. (`activity-log`: *Incremental, Ordered, Crash-Safe Log Persistence*)

## 3. Batched task-history backfill (openspec-core)

- [ ] 3.1 Before changing anything, capture the current `task_completion_history` output as a parity fixture. Build a temp repository with these commits: a check-off, a further check-off, an uncheck, a deletion and re-creation of `tasks.md`, a merge commit that touches `tasks.md` on both sides, an archive move, and two authors.
- [ ] 3.2 Rewrite `task_completion_history` in `crates/openspec-core/src/git.rs` as one `git log --all --reverse --since <since> --raw --no-abbrev -z --diff-merges=dense-combined --format=<RS>%H<US>%at<US>%an<US>%ae -- openspec/changes`. Parse post-image blob ids, taking the last id on `::` combined lines and the destination id on renames, and keep only paths accepted by `active_change_id_of_tasks_path`. (`activity-log`: *Fixed-Process Task-History Backfill*)
- [ ] 3.3 Read the blobs through one `git cat-file --batch` by generalising the existing private `cat_file` (`git.rs` ~1655) rather than adding a second helper: move its stdin write onto a writer thread joined after `wait_with_output`, so hundreds of ids cannot deadlock on full pipes, and update its "two short lines at most" comment. Feed it deduplicated object ids only. Parse the reply per object with the existing `blob_header_size` rule, treating a `<oid> missing` reply as 0 rather than failing the batch as `parse_blob_contents` does. Count completed tasks once per blob. An all-zero id counts as 0 without being read, and when no id needs reading the `cat-file` process is not started. Confirm `commit_file_blobs`'s existing tests still pass unchanged.
- [ ] 3.4 Keep the `last_completed` fold exactly as it is: positive deltas only, a per-path baseline, and the commit author attached.
- [ ] 3.5 Add these tests:
  - The fixture from 3.1 yields output equal to the captured pre-change result.
  - `invocation_log` records at most two git invocations for that repository however many commits it has. Run this case with at least 50 commits.
  - Deleting and then re-creating a `tasks.md` emits nothing for the deletion and the full count for the re-creation.
  - An empty window and a non-git directory return an empty `Vec`.
  - Pure unit tests of the raw-record parser on literal byte strings, with no git: a `::` combined merge line yields its last (result) object id, an `R`/`C` line yields the destination path and id, an all-zero id is a deletion, an archive path and a non-`tasks.md` path are dropped, and a commit header with an empty author yields `None`.
  - The per-object reply parser on literal bytes: a `missing` reply between two blobs yields 0 for it and the right counts for both neighbours.
  - `cat_file` with more ids than fit a pipe buffer (for example 2,000 copies of one blob id) completes.

## 4. Garden cache (openspec-core + openspec-app)

- [ ] 4.1 In `crates/openspec-core/src/git.rs`, replace `commit_log_authored` (~1056) with `commit_log_authored_checked(&RepoId, usize) -> Result<Vec<AuthoredCommit>, LifecycleError>`: `CommandFailed` for a failed spawn or non-zero exit, `InvalidUtf8` for undecodable output. Update the `lib.rs` re-export and the doc links in `garden.rs` (~159-165). Test that a missing git directory returns `Err(CommandFailed)` and that a real repository returns its commits with author, parents and decorations.
- [ ] 4.2 In `crates/openspec-core/src/garden.rs`, change `compute_garden` to take `&[AuthoredCommit]` and clone only the commits from today. Update its tests.
- [ ] 4.3 In `crates/openspec-core/src/repo_cache.rs`, add `pub type GardenCache = RepoCache<Arc<Vec<AuthoredCommit>>>` and update the module doc, which says "two derivations" share the cache, to name the garden as the third. Add a unit test that a `GardenCache` miss followed by a hit calls a counting miner once and returns the same `Arc` (`Arc::ptr_eq`). (`commit-garden`: *Commit Mining At Most Once Per History Change*)
- [ ] 4.4 In `crates/openspec-core/src/repo_monitor.rs`, make `RepoPaths::classify` mark `.git/worktrees/<name>/HEAD` and `.git/worktrees/<name>/logs/HEAD` as `graph`, in addition to their existing `status` and `reconcile` concerns. Confirm the watch on `worktrees/` reaches `logs/HEAD`. Add classify unit tests for both paths, and one for `worktrees/<name>/index`, which must stay status-and-reconcile only.
- [ ] 4.5 In `crates/openspec-app/src/service.rs`, add a `pub garden_cache: GardenCache` field to `AppService`, public like `lifecycle_cache` and `commit_activity_cache` (~210-214). Invalidate it in `spawn_lifecycle_cache_invalidator`'s `GraphChanged` arm and in its `Lagged` arm, and evict it on unregister next to `lifecycle_cache` and `commit_activity_cache` (~1344-1348).
- [ ] 4.6 Move the per-view loop of `commit_garden` into a named function, `garden_plots(views, today, &identity, &GardenCache, miner)`. It reads commits through `get_or_compute` and applies `today`, the identity, the label, `entry_key` and `active_count` after the cache on every call, then `sort_plots`. `commit_garden` calls it with `commit_log_authored_checked`.
- [ ] 4.7 Add `garden_plots` unit tests with a counting miner:
  - A second call with no invalidation does not mine.
  - Invalidating one of two repositories re-mines only that one.
  - A miner that returns `Err` is retried on the next call.
  - A different `today`, or an identity with an added alias, changes the plots with no re-mine.
  - `active_count` follows the views passed in, not the cached data.

  (`commit-garden`: *Commit Mining At Most Once Per History Change*)
- [ ] 4.8 In `crates/openspec-app/tests/dashboard.rs`, add the following tests and fix the existing race:
  - With `invocation_log`, two `commit_garden` calls with no history move issue no `log` invocation the second time.
  - After a commit, polling `commit_garden` shows the new commit (the pattern `graph_changed_re_mines_only_the_affected_repository` uses; the deadline only fails a hung test).
  - A commit on a detached HEAD in a linked worktree appears in the garden, polled the same way.
  - Unregistering evicts the garden entry: warm the garden, unregister, commit, re-register, and a single read (no polling, since no `GraphChanged` reaches a torn-down monitor) shows the commit.
  - Fix `commit_garden_is_computed_without_opt_in` (~173-182): in the post-midnight branch, poll `commit_garden` after the new commit until the plot is live instead of re-reading once, because a cached list now waits for the asynchronous `GraphChanged`. The deadline only fails a hung test.

## 5. App wiring and docs (openspec-app)

- [ ] 5.1 In `crates/openspec-app/src/service.rs` `bootstrap` (~262, ~323), replace `ActivityLog::load(config_dir.join("activity.json"))` with `ActivityLog::open(&config_dir)`. Port every other `ActivityLog::load` call site: the unit tests in `activity_log.rs` (~582-606, ~885, ~1001) and `crates/openspec-core/tests/watcher.rs` (~765).
- [ ] 5.2 Update the docs that describe `activity.json` as a contended second writer so they describe the activity log's lease instead: the module doc in `crates/openspec-app/src/review_progress.rs` (~14-17, coordinate with `atomic-settings-persistence` task 4.1, which edits the same file), the module doc in `crates/specforge-web/src/main.rs` (~14-17), and the *Concurrent worktrees* paragraph of the root `CLAUDE.md` ("concurrent instances co-write `activity.json`").
- [ ] 5.3 Add a test in `service.rs`: a bootstrap over a config directory holding a torn `activity.jsonl` with at least one good event does not run `backfill_activity` (the log is not empty). A bootstrap over an unparseable legacy `activity.json` leaves an `activity.json.corrupt-0` behind.
- [ ] 5.4 After this change is archived, update the Purpose paragraph of `openspec/specs/activity-log/spec.md`, which still names `activity.json`, to name `activity.jsonl`. Add a release-notes line saying that downgrading finds `activity.json.migrated`, which can be renamed back.

## 6. Verification

- [ ] 6.1 `bun install && bun run build` once in the worktree, so that `dist/` exists for `cargo test`.
- [ ] 6.2 `cargo fmt --all -- --check`
- [ ] 6.3 `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 6.4 `cargo test`
- [ ] 6.5 `bun run lint`
- [ ] 6.6 `bun run build`
- [ ] 6.7 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Fix every survivor with an assertion, or exclude it in `.cargo/mutants.toml` with a written reason. Never use `--baseline=skip`.
- [ ] 6.8 Smoke test with `specforge-serve` and `bun run dev`, against an isolated config directory seeded with a copy of a real `activity.json`:
  - The Dashboard's heatmap and streak match those from before the migration.
  - `activity.json.migrated` and `activity.jsonl` exist.
  - Checking off a task appends exactly one line.
  - Focusing the window and editing a spec file trigger no new `git log` for the garden. Watch with `GIT_TRACE` or `fs_usage`.
  - A commit appears in the garden within the debounce window.
- [ ] 6.9 Two-instance smoke test. With the desktop dev app (`bun run wt:dev`) and a standalone `specforge-serve` sharing the config directory, check off one task. `activity.jsonl` gains one line, not two. After quitting the desktop app, the next achievement is appended by `specforge-serve`.
- [ ] 6.10 `openspec validate cache-dashboard-history --strict`
