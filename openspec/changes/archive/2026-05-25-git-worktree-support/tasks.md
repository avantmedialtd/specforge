# Tasks

## 1. `openspec-core`: Git Plumbing

- [x] 1.1 Add a `git` module in `openspec-core` that wraps `std::process::Command::new("git")` with a small typed surface: `git_common_dir(path)`, `default_branch(repo_dir)`, `current_branch(worktree_path)`, `worktree_list(repo_dir)`
- [x] 1.2 Define types: `RepoId(PathBuf)`, `WorktreeInfo { path, branch, is_main, is_prunable }`, `DefaultBranch(String)`
- [x] 1.3 Implement default-branch detection cascade: `origin/HEAD` → `init.defaultBranch` → main-worktree's current branch → `None`
- [x] 1.4 Implement worktree enumeration that parses `--porcelain` output and tags the entry whose `git_common_dir == worktree_dir` as `is_main`
- [x] 1.5 Handle absence of `git` on PATH and non-zero exit codes by returning `None` (workspace degrades to flat), not by panicking
- [x] 1.6 Unit tests using temporary on-disk git repos: single-worktree repo, multi-worktree repo, repo with no remote, repo with a prunable worktree (path deleted under it)

## 2. `openspec-core`: Registry Origin and Discovery

- [x] 2.1 Add `WorkspaceOrigin` enum (`UserRegistered`, `Discovered { discovered_via: RepoId }`) to `WorkspaceFolder`; default to `UserRegistered` on deserialise via `#[serde(default)]`
- [x] 2.2 Modify the registry config-file serializer to write the `origin` field; readers tolerate its absence
- [x] 2.3 On `register(path)`: detect repo via the `git` module; if a repo is found, enumerate its worktrees and add the other ones as `Discovered`
- [x] 2.4 On `unregister(path)`: if removing a user-registered workspace that's the last user-registered entry for that repo, cascade-remove every `Discovered` workspace tagged with the same `discovered_via`
- [x] 2.5 On startup: load only `UserRegistered` entries from disk, then re-derive `Discovered` entries by scanning each repo's worktrees
- [x] 2.6 Unit tests for: register a repo with two worktrees, expect three workspaces; unregister the user-registered one, expect zero; restart-from-disk recomputes discovered set

## 3. `openspec-core`: Meta-Watcher and Default-Branch Watcher

- [x] 3.1 Install a non-recursive `notify` watcher on each repo's `.git/worktrees/` directory at the time the repo is first added to the registry
- [x] 3.2 Install a small watcher on `.git/config` and `.git/refs/remotes/origin/HEAD` to refresh the cached default branch
- [x] 3.3 On meta-watcher events: debounce briefly, re-run `worktree_list`, diff against the current `Discovered` set, add/remove watchers and registry entries, emit `InstanceAdded` / `InstanceRemoved` events
- [x] 3.4 Make the reconciler idempotent — running it twice with no on-disk change is a no-op
- [x] 3.5 Handle the `prunable` case: a worktree whose path is missing on disk is treated as removed even before `git worktree prune` runs
- [x] 3.6 Tear down meta-watcher and default-branch watcher when the last workspace for a repo is unregistered
- [x] 3.7 Unit / integration tests: simulate `git worktree add` and verify a new instance is registered without user action; simulate `rm -rf` of a worktree path and verify the instance is dropped

## 4. `openspec-core`: Aggregator (`repo_view.rs`)

- [x] 4.1 Define `WorkspaceView` (`Repo(RepoView)` / `Flat(WorkspaceFolder, Vec<ChangeData>)`), `RepoView`, `LogicalChange`, `ChangeInstance`, `DivergenceLabel` with serde `rename_all = "camelCase"`
- [x] 4.2 Implement the aggregator as a pure function: `(Vec<(WorktreeInfo, Vec<ChangeData>)>, Option<DefaultBranch>) → RepoView`
- [x] 4.3 Sort instances within a logical change by `modified_at` descending; tag the first as the primary
- [x] 4.4 Group archived-here vs active-here instances under the same logical change; classify the logical change as archived only when *all* its instances are archived
- [x] 4.5 Implement divergence detection: byte-compare every file under `openspec/changes/<name>/**` for the non-default instance against the default instance; return `Diverged`, `StaleVsArchived`, or `None`
- [x] 4.6 Cache divergence labels per `(repo_id, logical_change, instance_path)` with invalidation hooks the watcher can call when files change on either side
- [x] 4.7 Unit tests for the aggregator covering: single-instance, two-instance identical, two-instance diverged, archived-on-default-active-on-branch, no-default-branch, missing-instance edge cases

## 5. `openspec-core`: Cache Events and Tauri Command Surface

- [x] 5.1 Extend `CacheEvent` with `LogicalChangeAdded { repo_id, change_name }`, `LogicalChangeArchived { repo_id, change_name }`, `InstanceAdded { repo_id, change_name, worktree_path }`, `InstanceRemoved { repo_id, change_name, worktree_path }`
- [x] 5.2 Emit logical-level events from the aggregator after each refresh, deriving them by diffing the previous aggregated state against the new one
- [x] 5.3 Continue emitting existing `Updated` events at the instance grain so the detail-pane reactive-update behaviour keeps working
- [x] 5.4 Add a `get_workspace_views()` Tauri command returning `Vec<WorkspaceView>`; deprecate but keep `get_changes(workspace)` working for any callers not yet migrated
- [x] 5.5 Document the new event names and payload schemas alongside the existing `cache-updated` / `change-added` / `change-archived` constants
- [x] 5.6 Unit tests for event diffing: instance added without new logical change does not emit `LogicalChangeAdded`; archiving the last active instance emits `LogicalChangeArchived`

## 6. `specforge`: Tray Badge and Notifications

- [x] 6.1 Switch the badge computation to count non-archived logical changes across all `RepoView`s plus non-archived changes across all `Flat` workspaces; the badge is the sum
- [x] 6.2 Subscribe the notifications module to `LogicalChangeAdded` and `LogicalChangeArchived` only; ignore `InstanceAdded`, `InstanceRemoved`, and `Updated`
- [x] 6.3 Verify the tray-badge subscriber still fires on every relevant event (any logical-add or logical-archive triggers a badge recompute)
- [x] 6.4 Confirm `notifications_enabled` gating still works at the new event grain

## 7. Frontend: Types and Hook

- [x] 7.1 Mirror `WorkspaceView`, `RepoView`, `LogicalChange`, `ChangeInstance`, `DivergenceLabel` in `src/types.ts` with the exact camelCase wire shape
- [x] 7.2 Extend `TreeSelection` with `repo`, `logicalChange`, and `instance` variants
- [x] 7.3 Update `useWorkspaces` to consume `get_workspace_views()` instead of the flat workspace→changes map, and subscribe to the new logical and instance events
- [x] 7.4 Refetch the affected repo's view on any `LogicalChangeAdded` / `LogicalChangeArchived` / `InstanceAdded` / `InstanceRemoved` event
- [x] 7.5 Continue updating the detail pane on `Updated` events scoped to the currently-rendered instance

## 8. Frontend: Workspace Tree

- [x] 8.1 Render git-backed `WorkspaceView::Repo` entries as a top-level Repo group node, with the default branch name shown as a subtle annotation next to the repo name
- [x] 8.2 Render `WorkspaceView::Flat` entries with the existing single-workspace shape (unchanged behaviour for non-git workspaces)
- [x] 8.3 Inside a Repo, render `LogicalChange` rows: parent disclosure when `instances.len() >= 2`, flattened single instance row when `instances.len() == 1`
- [x] 8.4 Render each `ChangeInstance` with: branch name (or path basename fallback), task progress, modified-time, and divergence label chip when present
- [x] 8.5 Apply the ● active indicator to the primary instance of each logical change
- [x] 8.6 Handle the singleton → multi-instance promotion: when an `InstanceAdded` event takes a logical change from 1 to 2 instances, the row becomes a disclosure parent with both instances as children (preserve expand state for the previously-singleton row by defaulting to expanded the first time)
- [x] 8.7 Handle multi-instance → singleton collapse: when an `InstanceRemoved` event takes the count back to 1, render the remaining instance as a flat row
- [x] 8.8 Preserve expand/collapse state across re-renders for repo, logical-change, and the four-artifact subtree under each instance

## 9. Frontend: Detail Pane and Click Behaviour

- [x] 9.1 `repo` selection: no-op (deferred-interaction node, parallel to today's workspace click)
- [x] 9.2 `logicalChange` selection: no-op (the parent disclosure row is not selectable in Sketch 2c)
- [x] 9.3 `instance` selection: render the instance's `proposal.md` by default (or the most-recently-modified leaf artifact if we have it cheaply), matching today's "click change does nothing, click an artifact renders it" behaviour
- [x] 9.4 All leaf-artifact rendering reads from the selected instance's `worktree_path`, not from a workspace-level path
- [x] 9.5 If the currently-selected instance disappears (worktree pruned), fall back to the primary instance of the same logical change, or clear the detail pane if the logical change is gone

## 10. Verification

- [ ] 10.1 Register a git repo with two worktrees; the tree shows one Repo group, with logical changes whose instances render under disclosure parents (multi) or flat (single)
- [ ] 10.2 Run `git worktree add` from a terminal after the app is open; within seconds the tree updates to include the new worktree as a discovered instance without user action
- [ ] 10.3 `rm -rf` a discovered worktree path; the instance disappears from the tree within the debounce window
- [ ] 10.4 Edit `proposal.md` in one worktree only; the `[diverged]` label appears on that worktree's instance
- [ ] 10.5 Archive a change on the default branch while it's still active on a feature branch; `[stale]` appears on the feature-branch instance, the logical change stays in the Active section
- [ ] 10.6 Archive the change on the feature branch too; the logical change moves to the Archive section and a `LogicalChangeArchived` notification fires
- [ ] 10.7 Badge: a change touched by three worktrees still increments the badge by 1; archiving every instance decrements by 1
- [ ] 10.8 Notifications: a Claude harness job creates an ephemeral worktree that touches an existing change — no notification fires (logical change already existed). A harness job creates a new change ID — notification fires once
- [ ] 10.9 Register a non-git workspace; it renders flat with no Repo grouping, no instances, no divergence labels
- [x] 10.10 Run `openspec validate git-worktree-support --strict` and confirm zero validation errors
