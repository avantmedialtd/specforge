# Refresh Tasks Artifact and Task Counts on Write

## Why

The workspace tree's Tasks artifact row and per-instance task progress count lag behind on-disk reality:

- **Tasks artifact dims after creation.** When a change is scaffolded by `/opsx:ff`, the Tasks artifact row remains rendered as "absent" (dim, non-interactive) after `tasks.md` is written. The row only flips to "present" on a subsequent unrelated edit to `tasks.md`.
- **Checked-state edits don't propagate.** When a checkbox in `tasks.md` is toggled (e.g. via Claude Code's `Edit`), the instance row's `(completed/total)` progress and the per-section completion glyph don't update; they only catch up on a later, separate event.

Both symptoms violate `spec-browser`'s *Reactive Updates from Filesystem* requirement that the tree must reflect on-disk changes within the watcher's debounce window. The likely root cause is an ordering race in the watcher pipeline: the event forwarder (which fires the `cache-updated` Tauri event the frontend listens for) and the aggregator (which recomputes the cached `last_views` snapshot that `get_workspace_views` returns) are independent subscribers of the same broadcast channel. When the forwarder wakes first, the frontend re-fetches before the aggregator has refreshed `last_views`, and so it sees the previous snapshot. The UI is consistently one event behind on any change the aggregator's diff doesn't itself surface as a follow-up event — which includes every artifact-row presence flip and every task-count update inside an already-tracked change.

## What Changes

- Re-order the watcher pipeline so the aggregated `last_views` snapshot is refreshed **before** any `cache-updated` / `change-added` / `change-archived` Tauri event reaches the frontend. After this change, the frontend's first re-fetch on any event always sees the post-event state.
- Add two scenarios to `spec-browser`'s *Reactive Updates from Filesystem* requirement that explicitly cover the symptoms: a Tasks artifact row flipping from dim to present when `tasks.md` is created, and the instance row's progress count updating when a checkbox is toggled — both within the debounce window, with no second event needed.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `spec-browser`: the *Reactive Updates from Filesystem* requirement gains two new scenarios that close the gap left by today's wording — which covers detail-pane content and structural change-added/archived events, but not artifact-row presence or in-place task-count changes inside an existing change.

## Impact

- `crates/openspec-core/src/watcher.rs` — `handle_events` and `spawn_aggregator`/`aggregate_and_emit` are reordered so the cache update, aggregated-view refresh, and event emission happen as one ordered sequence per debounced batch. The aggregator no longer races the forwarder.
- `crates/openspec-core/src/repo_monitor.rs` — `reconcile`'s `emit(CacheEvent::Updated)` calls are folded into the same ordering so worktree-discovery events behave consistently with file-change events.
- `openspec/specs/spec-browser/spec.md` — two new scenarios under *Reactive Updates from Filesystem*.
- Frontend (`src/hooks/useWorkspaces.ts`, `src/components/WorkspaceTree.tsx`) is unchanged — the contract it consumes (`cache-updated` event, `get_workspace_views` returning fresh data) is unchanged, just made honest.
- No IPC shape changes; no new commands; no settings migration.
