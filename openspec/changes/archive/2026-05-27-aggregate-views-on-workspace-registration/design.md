# Design

## Decision: Call `aggregate_and_emit()` from the IPC handlers

Both `register_workspace` and `unregister_workspace` already mutate the registry, the watcher map, and the cache. We add one synchronous call to `watcher.aggregate_and_emit()` after those mutations and before the handler returns. The call recomputes `last_views` from the current registry + cache, diffs it against the previous snapshot, and emits any `LogicalChange*` / `Instance*` events the diff implies — exactly what happens after a real filesystem event picked up by the aggregator task.

Why this works:

- `aggregate_and_emit` is the single recomputation primitive (`crates/openspec-core/src/watcher.rs:196`) and is idempotent.
- It is the same call `lib.rs:92` makes at startup after the initial set of workspaces is added, which is exactly why a restart papers over the bug today.
- `get_workspace_views` reads `last_views` directly (`watcher.rs:173`), so updating that snapshot in-line is sufficient and complete.

For `unregister_workspace`, the existing code already loops over the `removed` paths calling `watcher.remove_workspace(p)`. A single `aggregate_and_emit()` after the loop suffices: `last_views` is recomputed once from the final settled state, regardless of how many paths were dropped in the cascade.

## Alternatives Considered

### A. Introduce `CacheEvent::WorkspaceAdded` and emit `WorkspaceRemoved` from explicit unregister

Add a new event variant for adds and have `WatcherManager::add_workspace` / `remove_workspace` emit one. The aggregator task would pick them up naturally and recompute `last_views` on the next tokio tick.

Rejected: more surface area for the same outcome. A new variant has to be threaded through `is_raw_event`, the event forwarder in `events.rs`, the IPC event name, and the TypeScript event payload mirrors. The bug is localised to two IPC handlers; a new event variant is overreach. Worth revisiting only if a *future* feature needs an out-of-band notification that the tracked-workspace set changed (e.g. extension hooks).

### C. Recompute views on demand inside `get_workspace_views`

Drop the `last_views` cache and have `get_workspace_views` call `compute_views(...)` directly each request.

Rejected: `last_views` also drives `total_active_logical_count` for the tray badge (`watcher.rs:181`) and is the input to the diffing pass that emits `LogicalChange*` / `Instance*` events. Killing the cache means re-architecting those paths too. Keeping `last_views` as the single source of truth and ensuring it is fresh at the right moments is the smaller change.

## Why the bug existed in the first place

`last_views` was added as a cache driven by the watcher event stream. Edits to files *inside* an already-tracked workspace produce a `CacheEvent::Updated` (or `ChangeAdded`/`ChangeArchived`) which feeds the aggregator. But changes to the *set* of tracked workspaces themselves — registration and unregistration — went through a separate code path (`WatcherManager::add_workspace` / `remove_workspace`) that mutates the cache directly without sending any event. There is no `CacheEvent::WorkspaceAdded` variant; `CacheEvent::WorkspaceRemoved` exists but is emitted only by `RepoMonitor::reconcile` when git itself reports a worktree as gone, not by explicit user removal.

The startup path got it right because `lib.rs:92` calls `aggregate_and_emit()` once after the initial population. The runtime IPC handlers never replicated that step.

## Concurrency

`aggregate_and_emit` acquires the registry mutex and then the cache `RwLock` internally. Both IPC handlers release their own registry lock before the call lands (the lock guards are scoped to the registry-mutation block and drop at its end). No nested-lock or deadlock risk.

The call is synchronous and returns before the IPC handler returns. By the time the frontend's `refresh()` proceeds to `getWorkspaceViews()`, `last_views` is already the post-registration snapshot.

## Out of Scope

- Any change to `WatcherManager`'s internal API, `CacheEvent`, or the IPC contract.
- Any change to the frontend's `useWorkspaces` hook or `SettingsView` handlers.
- The pre-existing wrinkle that the watcher only emits `WorkspaceRemoved` from `RepoMonitor::reconcile`. That path remains the only emitter; explicit unregistration is now covered by the synchronous aggregation call instead of by an event.
