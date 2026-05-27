## 1. Refactor the refresh helper

- [x] 1.1 Move the body of `WatcherManager::aggregate_and_emit` (the `compute_views` + `diff_views` + `last_views.write` + emit-diff sequence) into a synchronous helper on `Inner` that returns the `Vec<CacheEvent>` it would emit, instead of broadcasting them itself
- [x] 1.2 Reduce `WatcherManager::aggregate_and_emit` to a thin wrapper that calls the `Inner` helper and then broadcasts the returned events — preserving the existing public signature for any external callers

## 2. Wire the helper into `handle_events`

- [x] 2.1 In `Inner::handle_events`, after `self.cache.write().unwrap().insert(...)` and before any `event_tx.send(...)` call, invoke the new `Inner` helper to refresh `last_views` and collect the diff events it produces
- [x] 2.2 Keep the existing structural send order (`ChangeAdded` per added id → `ChangeArchived` per removed id → `Updated`), then send the collected diff events after `Updated`, preserving the observable ordering on the broadcast channel
- [x] 2.3 Verify in a focused unit test under `crates/openspec-core/tests/watcher.rs` that an edit to `tasks.md` produces an `Updated` event whose post-recv `WatcherManager::workspace_views()` already reflects the new `completedTasks` count

## 3. Wire the helper into `RepoMonitor::reconcile`

- [x] 3.1 In `repo_monitor::reconcile`, immediately before each `watcher.emit(CacheEvent::Updated { workspace: ... })` for a freshly-added worktree, call the same refresh helper via the `WatcherManager` wrapper so worktree-discovery batches inherit the same ordering guarantee
- [x] 3.2 Add a watcher test that registers a workspace, creates a new change with only `.openspec.yaml`, then creates `tasks.md` inside it, and asserts that the next `Updated` event corresponds to `artifacts.tasks == true` in `workspace_views()`

## 4. Retire the aggregator subscription

- [x] 4.1 Remove `WatcherManager::spawn_aggregator` and its `is_raw_event` helper
- [x] 4.2 Remove the `watcher_for_setup.spawn_aggregator();` call and surrounding comment block in `crates/specforge/src/lib.rs`
- [x] 4.3 Update the doc comment on `WatcherManager::emit` to state that callers emitting raw cache events MUST refresh the aggregated view first (via the wrapper), and that the broadcast channel no longer has an aggregator subscriber

## 5. Validation

- [x] 5.1 Run the full Rust test suite (`cargo test`) and confirm `openspec-core` and `specforge` crates both pass, including the two new tests added in 2.3 and 3.2
- [x] 5.2 Run `bun run build` to confirm the frontend type-checks against any unchanged IPC shapes
- [ ] 5.3 Start `bun tauri dev` (per `feedback_run_app_yourself`) and reproduce the original bug scenarios: (a) scaffold a fresh change via `/opsx:ff` in a sandbox workspace and confirm the Tasks artifact row renders as present on first paint after `tasks.md` is written; (b) toggle a `tasks.md` checkbox via Claude's `Edit` tool and confirm the instance row's `(n/N)` count updates on the first refresh — no second edit required
