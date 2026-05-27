# Aggregate Views on Workspace Registration

## Why

After registering or unregistering a workspace from the Settings view, the workspace tree pane keeps showing the old shape until either an unrelated cache event fires (a file change somewhere) or the app is restarted. The Settings list updates correctly because it reads the registry directly. The tree pane reads a cached `last_views` snapshot inside `WatcherManager` that is only recomputed in response to raw `CacheEvent`s — and neither `register_workspace` nor `unregister_workspace` emits one. The same bug shows up symmetrically for adds (new workspace missing from the tree) and removes (stale entry lingers).

## What Changes

- `register_workspace` in `crates/specforge/src/commands.rs` SHALL call `watcher.aggregate_and_emit()` synchronously after the existing per-folder `add_workspace` loop and `sync_repos()` call, before returning the `RegisteredWorkspace`.
- `unregister_workspace` in the same file SHALL call `watcher.aggregate_and_emit()` once after the loop that tears down watchers for the removed paths. A single call covers the cascade case (a user-registered workspace plus any discovered worktrees of the same repo dropped together).
- The post-condition — that the aggregated view returned by `get_workspace_views` reflects the post-registration set as soon as the IPC command returns — SHALL be pinned down in the `workspace-registry` capability spec so the bug cannot recur in spirit.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `workspace-registry`: Adds an "Aggregated View Freshness on Registration Change" requirement with scenarios that make the freshness contract explicit for register, unregister, and cascade unregister.

## Impact

- Rust: `crates/specforge/src/commands.rs` only. No `openspec-core` changes, no new `CacheEvent` variant, no IPC contract or TypeScript-type changes.
- Frontend: unchanged. `useWorkspaces.refresh()` already calls `getWorkspaceViews()` after the command resolves; once `last_views` is fresh on return, the tree updates without any new wiring.
- Tests: no test forced to change. A small Rust integration test exercising the IPC handlers' post-condition is nice-to-have if existing test infrastructure makes it cheap, otherwise the manual verification step in tasks.md is sufficient.
