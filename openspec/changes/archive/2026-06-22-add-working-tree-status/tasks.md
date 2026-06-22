# Tasks — Working-Tree Status Indicators

## 1. Core: status computation
- [x] 1.1 Add `SpecCommitState { Committed, Modified, Untracked }` and a
  `WorktreeStatus { dirty: bool, spec_states: HashMap<String, SpecCommitState> }`
  result type to `git.rs` (camelCase serde where it crosses IPC).
- [x] 1.2 Implement `git::worktree_status(worktree: &Path, change_ids: &[String])`
  running `git status --porcelain --untracked-files=all` through `git_command`,
  parsing per the precedence rules (Modified > Untracked > Committed), handling
  rename `->` and quoted paths.
- [x] 1.3 Unit-test the porcelain parser: clean, untracked-only spec, modified
  tracked spec, mixed (→ Modified), rename, non-spec dirt only (→ dirty but all
  specs Committed), submodule summary line.

## 2. Core: thread into the view
- [x] 2.1 Add `spec_commit_state: SpecCommitState` to `ChangeInstance` and
  `dirty`, `dirty_worktrees`, `has_uncommitted_specs` to `RepoView`.
- [x] 2.2 In `compute_views`, call `worktree_status` once per worktree (beside
  `current_branch`), build the status map, attach it to `WorktreeSnapshot`.
- [x] 2.3 In `build_repo_view` (pure), set each instance's `spec_commit_state`
  from the precomputed status and roll up `dirty` / `dirty_worktrees` /
  `has_uncommitted_specs` onto the `RepoView`.
- [x] 2.4 Extend `repo_view.rs` tests: instance states + repo rollup, including a
  dirty-but-no-spec-changes worktree and a clean repo.

## 3. Core: freshness
- [x] 3.1 Add a `.git/index` debounced watcher to `RepoMonitor` that triggers a
  re-aggregation on change.
- [x] 3.2 Verify the WSL backend path (`git_command` chokepoint) is used; no
  direct `git.exe` call introduced.

## 4. App / IPC
- [x] 4.1 Ensure the new fields serialize over existing `get_workspace_views`
  (or equivalent) — no new command needed if they ride the existing `RepoView`.
- [x] 4.2 Recompute views on main-window focus (Tauri focus event → refresh).

## 5. Frontend
- [x] 5.1 Mirror `SpecCommitState` and the new `RepoView`/`ChangeInstance`
  fields in `src/types.ts`.
- [x] 5.2 Render the per-instance commit-state chip in `WorkspaceTree.tsx`
  beside the divergence chip (only for Modified / Untracked).
- [x] 5.3 Render the repo-node dirty dot + specs-uncommitted mark, suppressed
  when clean; tooltip lists dirty worktrees.
- [x] 5.4 CSS for chips/dots; verify against the existing chip styling.

## 6. Verify
- [x] 6.1 `cargo test` (core) green, including new parser + rollup tests.
- [x] 6.2 `bun run build` (tsc) clean — types mirrored.
- [~] 6.3 Manual visual check. The dev app (`bun run wt:dev`) builds and
  launches cleanly with these changes, and the `specforge` repo + this worktree
  (which carries the untracked `add-working-tree-status` change) are the live
  fixture. Pixel confirmation of the chip + rollup dots is still pending — it
  needs a foreground session; screen capture is blocked in this background run.
