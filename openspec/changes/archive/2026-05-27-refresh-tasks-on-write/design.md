## Context

The cache-event pipeline in `crates/openspec-core/src/watcher.rs` has two parallel consumers of the same `broadcast::Sender<CacheEvent>`:

1. **Event forwarder** (`crates/specforge/src/events.rs::spawn_event_forwarder`) — translates each `CacheEvent` into the matching Tauri event (`cache-updated`, `change-added`, …). The frontend's `useWorkspaces` hook listens for these and calls `getWorkspaceViews()` whenever one fires.
2. **Aggregator subscriber** (`WatcherManager::spawn_aggregator`) — on every raw event recomputes `last_views`, the cached snapshot that `get_workspace_views` returns, and emits any derived `LogicalChange*` / `Instance*` events that fall out of the diff.

Both are independent tokio tasks. After `handle_events` updates the cache and broadcasts `Updated`, the order in which the forwarder and the aggregator resume their `recv().await` is unspecified. When the forwarder wins:

```
handle_events                forwarder                       aggregator
─────────────                ─────────                       ──────────
parse + cache.insert
broadcast Updated  ─────────►rx.recv() returns Updated       (still parked)
                             emit "cache-updated"
                                                  ───────►frontend re-fetches
                                                  ◄───────reads STALE last_views
                                                          (artifacts.tasks=false,
                                                           old completion count)
                                                             rx.recv() returns Updated
                                                             recompute, last_views updated
                                                             (no diff event emitted)
```

The frontend has now received its only chance to refresh for this debounce batch — and it saw the *pre*-event snapshot. Subsequent debounce batches each push the UI one step closer to truth (the aggregator's previous-batch write becomes the frontend's current-batch read), but the UI is structurally always one event behind on any state change the aggregator's diff doesn't surface as a follow-up event.

`diff_views` emits a `LogicalChangeAdded` / `LogicalChangeArchived` / `InstanceAdded` / `InstanceRemoved` only on structural transitions of `(repo_id, change_name)` tuples or their `worktree_path` sets. It deliberately does *not* fire on content changes inside an existing instance — so artifact-row presence flips (`artifacts.tasks` flipping `false → true` when `tasks.md` is created inside an already-tracked change) and task-count edits (a checkbox toggle) get no second event and no second fetch, leaving the UI stuck on the stale snapshot.

This is consistent with both reported symptoms:

- **Bug #1** (Tasks dim after `/opsx:ff`): the change directory was already tracked (added by an earlier debounce batch when only `.openspec.yaml` existed) so creating `tasks.md` is a content change, not a structural one. Frontend reads `last_views` before the aggregator catches up; sees `artifacts.tasks = false`. The next *unrelated* edit to `tasks.md` triggers another batch — and the frontend's read on *that* batch sees the previous batch's now-updated `last_views`, finally showing `artifacts.tasks = true`.
- **Bug #2** (checkbox edits don't propagate): toggling a `[ ]` to `[x]` is purely a content change. Same race, same one-event lag.

## Goals / Non-Goals

**Goals:**

- After any debounced batch of filesystem events, the *first* `cache-updated` / `change-added` / `change-archived` Tauri event the frontend sees must correspond to a `last_views` snapshot that already reflects that batch. No "one event behind" lag.
- The fix must cover both `handle_events` (file edits inside `openspec/changes/`) and `RepoMonitor::reconcile` (worktree add/remove emits `Updated` directly via `WatcherManager::emit`), since both can produce content-only batches that the aggregator's diff doesn't follow up on.
- The frontend contract is unchanged: it still listens on the same Tauri events and still calls `get_workspace_views`. No IPC shape changes.

**Non-Goals:**

- Restructuring the parser, the cache, or the aggregated-view diff algorithm. The bug is in *ordering*, not in any of these components' contents.
- Eliminating the broadcast channel. Multiple subscribers (forwarder, badge updater, notifications dispatcher, dock badge, tray glyph updater) are a legitimate use of `broadcast::Sender`; we only need to guarantee a precondition before the first send for a given batch.
- Adding a new "fully-refreshed" event variant. The existing events already imply post-batch state; making that implication true is enough.

## Decisions

### Decision 1: Refresh `last_views` synchronously inside `handle_events` *before* broadcasting raw events.

The aggregation is a synchronous function (`compute_views` + `diff_views` + writing `last_views`). Calling it from inside `handle_events` after the cache write but before the broadcast costs one extra `compute_views` per batch on the watcher task — measured in microseconds for any realistic workspace count. In exchange, every subscriber observes a consistent post-batch `last_views` when it wakes.

Concretely, `Inner::handle_events` will:

1. Filter + dedupe events as today.
2. Re-parse via `parse_all_changes`.
3. `cache.insert(workspace, new_changes)` as today.
4. **New**: call a synchronous refresh — `Inner::refresh_aggregated_view(...)` — that recomputes `last_views` and returns any `diff_views` events to emit.
5. Send the structural events (`ChangeAdded` / `ChangeArchived`) and `Updated` as today.
6. Send the derived events returned by step 4 (`LogicalChangeAdded`, `LogicalChangeArchived`, `InstanceAdded`, `InstanceRemoved`).

The same refresh helper is invoked by `RepoMonitor::reconcile` immediately before its `watcher.emit(CacheEvent::Updated { … })` call so worktree-discovery batches get the same guarantee.

**Why this ordering, not "emit first, refresh second":** The whole point is that the frontend's read happens through `cache-updated` → `get_workspace_views`. Refreshing after the emit would re-introduce the race; refreshing before makes the emit a true announcement of completed work.

**Alternatives considered:**

- *Wait for the aggregator inside the forwarder*: keep `spawn_aggregator` as the canonical refresh path and have the forwarder block until the aggregator signals completion (e.g. via a `tokio::sync::Notify` or a watch channel). Rejected: introduces a backchannel between two subscribers that shouldn't know about each other, and forwarder latency becomes coupled to aggregator scheduling.
- *Make `get_workspace_views` recompute on every call*: cheaper code change but adds `parse_all_archived` I/O to every frontend refresh, which scales with worktree count and can be hit several times per debounce batch. Also re-introduces a (smaller) race window because the recompute reads `cache`, which is also written by `handle_events` — though the existing `RwLock` makes this safe. Rejected on perf grounds.
- *Add a new "views-updated" Tauri event the aggregator fires after writing `last_views`, and have the frontend listen for that instead of `cache-updated`*: clean, but requires frontend changes for a backend ordering bug, and the existing `cache-updated` event still needs ordering guarantees for the badge updater and the notifications dispatcher which read state too. Rejected.

### Decision 2: Retire `spawn_aggregator`'s broadcast subscription.

Once both `handle_events` and `RepoMonitor::reconcile` call the synchronous refresh helper before emitting, the aggregator subscription becomes a duplicate path with weaker ordering. Keeping it would mean every raw event triggers the refresh twice — once synchronously (fast path, correct order) and once a moment later via broadcast (no-op because `last_views` is already current, but extra work and a confusing second code path). Removing the subscription leaves one canonical refresh trigger.

`WatcherManager::spawn_aggregator` is removed; the `lib.rs` setup that called it is removed. `aggregate_and_emit` is reduced to a thin wrapper around the same helper for any future code that needs an explicit "recompute now" hook (e.g., tests).

**Alternative:** keep `spawn_aggregator` for defence-in-depth in case a new emit site is added in the future without calling the refresh helper. Rejected — the cost of forgetting (another lag bug) is the same either way; making the contract explicit via the helper is preferable to a silent safety net that masks problems.

### Decision 3: Refresh helper is on `Inner`, not `WatcherManager`.

`Inner` already owns `cache`, `last_views`, `event_tx`, and `registry` — everything `compute_views` and `diff_views` need. Moving `aggregate_and_emit`'s body into `Inner::refresh_aggregated_view` lets `handle_events` call it via `&self` without any new clone/Arc gymnastics. `WatcherManager` retains a thin public method that forwards to `Inner` for external callers (`RepoMonitor::reconcile`, tests).

## Risks / Trade-offs

- **[Risk] The synchronous refresh adds latency to every debounce batch.** → Mitigation: `compute_views` is cheap (a few `BTreeMap` builds and an `mtime` walk per worktree; no I/O beyond what the existing aggregator already does); `parse_all_archived` runs once per worktree per batch as it does today. The watcher task is not on any user-interactive path. Measured impact expected to be < 1 ms per batch for typical workspaces; if a future profiling pass shows otherwise, the helper can be made async and `tokio::spawn_blocking`'d without changing the ordering invariant.
- **[Risk] A future code path emits a raw event without calling the refresh helper first, re-introducing the bug silently.** → Mitigation: there are only two emit sites today (`handle_events`, `RepoMonitor::reconcile`); a doc comment on `WatcherManager::emit` will note the contract. A unit test seals the invariant for `handle_events` by asserting that a file edit emits `Updated` *and* that `workspace_views()` already reflects the post-edit state at the moment that event is observed (subscribe + `await recv` + check `last_views`).
- **[Trade-off] `aggregate_and_emit`'s diff events now interleave with `handle_events`'s structural events.** Today the order on the broadcast channel for a batch that creates a new change is `ChangeAdded → Updated → LogicalChangeAdded → InstanceAdded` (the last two emitted by the aggregator after the first two). The new ordering is the same — diff events still fire after `Updated` — because step 6 above runs after step 5. No frontend code relies on the inter-event order beyond "process each as it arrives", so this is observationally equivalent.

## Migration Plan

No data migration. The change is a pure runtime ordering fix; no on-disk format, no settings shape, no IPC schema changes. A clean rebuild + relaunch is sufficient.
