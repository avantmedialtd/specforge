# Tasks — Temporarily Disable Workspaces

## 1. Presentation store gains the disabled flag

- [ ] 1.1 Add `disabled: bool` to `PresentationEntry` in `crates/openspec-core/src/presentation.rs`, with `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` so enabled rows add no key to `presentation.json` and files written before this change load as enabled (`workspace-registry`: *Workspace Disable State*).
- [ ] 1.2 Amend `PresentationEntry::is_empty` in the same file so an entry is empty only when it has no display name, no colour, **and** is not disabled — otherwise a disabled-only entry is pruned on save and the flag is lost on restart (`workspace-registry`: *Workspace Presentation Persistence*).
- [ ] 1.3 Add `WorkspacePresentationStore::set_disabled(key, bool)` as a read-modify-write that preserves the entry's existing display name and colour, and leave the existing `set` signature untouched so a name/colour edit cannot clobber the disabled state (`workspace-registry`: *Workspace Presentation Persistence*).
- [ ] 1.4 Add `WorkspacePresentationStore::is_disabled(&PresentationKey) -> bool` and extend `lookup` (or add a sibling accessor) so IPC join sites can read all three fields in one pass.
- [ ] 1.5 Unit-test in `presentation.rs`: a disabled-only entry survives a save/reload round trip; `set_disabled` preserves name and colour; `set` preserves the disabled state; a legacy file with no `disabled` key loads as enabled; clearing name and colour on a disabled entry retains it.

## 2. Cold aggregation of disabled rows

- [ ] 2.1 Add a `cold: bool` field to `RepoGatherInput` in `crates/openspec-core/src/repo_view.rs`, defaulted false.
- [ ] 2.2 Thread an `is_disabled: impl Fn(&PresentationKey) -> bool` predicate parameter through `gather_views`, matching the existing `default_branch_for` closure style, and set `cold` on each repo slot from it. Flat slots record the same flag for the aggregator.
- [ ] 2.3 In `compute_repo_rows_pooled` and `compute_repo_snapshot`, skip every git job for a cold row: no `worktree_list`, no `worktree_branch_and_status`. Keep the archived-stub `read_dir` (`parser::list_archived_stubs`, called from `compute_worktree`) — the Dashboard's archived counts and today's ships depend on it (`workspace-registry`: *Cold Aggregation of Disabled Rows*).
- [ ] 2.4 Make `resolve_main_worktree` use its documented no-subprocess fallback ("path matching the parent of the common dir") directly for cold rows, so `RepoView::name` stays correct without spawning `git worktree list`.
- [ ] 2.5 Add a `disabled: bool` to `RepoView` and to the `WorkspaceView::Flat` variant, populated during `aggregate` from the same predicate. Mark it `#[serde(default, skip_serializing)]` — disabled rows are filtered out before reaching any frontend, so the flag is an in-process concern only and needs no `src/types.ts` mirror.
- [ ] 2.6 Update `compute_views`, `gather_repo_view`, and `compute_repo_view` to accept and forward the predicate so the scoped and full recompute paths cannot drift.
- [ ] 2.7 Unit-test in `repo_view.rs`: a cold row reports accurate active/archived counts and task rollups; its `dirty`, `dirty_worktrees`, `has_uncommitted_specs`, `default_branch`, and per-instance `branch`/`spec_commit_state` hold defaults; the row keeps its config position and display name.
- [ ] 2.8 Add an integration test in `crates/openspec-core/tests/` asserting via `git::invocation_log` that recomputing a registry with one enabled and one disabled repository invokes git for the enabled repository's worktrees and **not** for the disabled one (`workspace-registry`: *Cold Aggregation of Disabled Rows*).

## 3. Watcher, badge count, and re-enable path

- [ ] 3.1 Give `WatcherManager` a disabled-set handle in `crates/openspec-core/src/watcher.rs`, following the existing `set_activity_log` hook pattern, so `Inner`'s recompute paths can supply the predicate from §2.
- [ ] 3.2 Filter `total_active_logical_count` by the per-view `disabled` flag so the tray badge excludes disabled rows (`tray-indicator`: *Active-Change Badge*).
- [ ] 3.3 Confirm `add_workspace` / `remove_workspace` are untouched and add a regression test asserting that disabling a workspace changes neither `watched_count()` nor `repo_monitor_count()`, and that its cache entry continues to update on a filesystem change (`workspace-registry`: *Disabled Workspaces Continue To Be Watched*).
- [ ] 3.4 Add a test asserting `diff_achievements` still records achievements for a disabled workspace's batch — the activity-log write happens before the cache insert and must stay upstream of the view filter (`workspace-registry`: *Disabled Workspaces Continue To Be Watched*).

## 4. Shell commands and wiring

- [ ] 4.1 In `crates/openspec-app/src/service.rs`, supply the presentation store's disabled predicate to the watcher's recompute paths at startup and after every registration change.
- [ ] 4.2 Add `AppService::set_workspace_disabled(uri, repo_id, disabled)` that writes through `set_disabled`, then runs the scoped `refresh_aggregated_view_for(repo_id)` (falling back to the full refresh for a flat workspace) **before returning**, so the next view request is already warm (`workspace-registry`: *Re-enable Freshness*).
- [ ] 4.3 Add the `set_workspace_disabled` `#[tauri::command]` in `crates/specforge/src/commands.rs`, mirroring `set_workspace_presentation`'s `uri` + optional `repo_id` key selection, and emit `workspace-presentation-updated` on success. Register it in the handler list in `crates/specforge/src/lib.rs`.
- [ ] 4.4 Filter disabled rows out of `get_workspace_views` in `crates/specforge/src/commands.rs`, after the presentation join, so the desktop, web, and terminal frontends all inherit the exclusion from one place (`workspace-registry`: *Disabled Rows Excluded From the Tree Pane*).
- [ ] 4.5 Join the disabled state into `list_workspaces` in the same file, using the existing `PresentationKey::{Repo,Flat}` selection so sibling worktrees of one repository report a shared state (`workspace-registry`: *Presentation Fields on Listed Workspaces*).
- [ ] 4.6 Add `disabled: bool` to `RegisteredWorkspace` in `crates/openspec-core/src/types.rs` with `#[serde(rename_all = "camelCase")]` already in force, and hand-mirror the field on the `RegisteredWorkspace` interface in `src/types.ts` in this same task — the type crosses the IPC boundary and there is no codegen.
- [ ] 4.7 Verify `get_dashboard` reads `watcher.workspace_views()` directly and is **not** routed through the filtered command path, so disabled workspaces keep contributing to every Dashboard figure (`dashboard`: *Dashboard Unaffected by Workspace Disable*).
- [ ] 4.8 Add a shell-level test that `get_workspace_views` omits a disabled row while `list_workspaces` still returns it marked disabled.

## 5. Notification suppression

- [ ] 5.1 In `crates/specforge/src/notifications.rs`, suppress `LogicalChangeAdded` and `LogicalChangeArchived` dispatch for logical changes whose repository is disabled, resolving the state through the presentation store (`tray-indicator`: *Desktop Notification on New Change*, *Desktop Notification on Archive Transition*).
- [ ] 5.2 Ensure suppression is dispatch-only: the event still flows on the broadcast channel and the achievement is still recorded, so re-enabling does not replay suppressed notifications and the Dashboard's shipped haul is unaffected.
- [ ] 5.3 Test that a new logical change and a final-instance archive in a disabled workspace dispatch no notification while still recording their achievements.

## 6. Settings toggle in the frontend

- [ ] 6.1 Add the `setWorkspaceDisabled` wrapper to `src/api.ts` via `invokeLogged`, matching the existing `setWorkspacePresentation` signature.
- [ ] 6.2 Add a per-row enabled/disabled toggle to `WorkspaceRow` in `src/components/SettingsView.tsx`, beside the display-name field and palette swatches, reading its state from the listed workspace's `disabled` field and refetching via `onWorkspacesChanged` after a successful call (`workspace-registry`: *Settings View*).
- [ ] 6.3 Render a disabled row's visual state in the Settings list (muted styling plus an accessible label) while keeping its remove, rename, and swatch controls fully operable.
- [ ] 6.4 Add the Dashboard copy noting that its totals include disabled workspaces, so the Dashboard-vs-tree count difference is legible (`dashboard`: *Dashboard Unaffected by Workspace Disable*).

## 7. Verification

- [ ] 7.1 Run `bun run build` first in a fresh worktree (strict `tsc --noEmit` plus the bundle; `cargo test` fails workspace-wide until `dist/` exists).
- [ ] 7.2 Run the focused Rust suites: `cargo test -p openspec-core --test registry`, `--test watcher`, and the new cold-aggregation integration target.
- [ ] 7.3 Run `cargo test` across the workspace.
- [ ] 7.4 Run `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff` and resolve every survivor — the predicate threading in `repo_view.rs` and `watcher.rs` is exactly the shape that produces them (a flipped cold check passes any test that only asserts cache-derived fields). Add the missing assertion rather than widening a margin; record a written reason in `.cargo/mutants.toml` only where a mutant is genuinely untestable.
- [ ] 7.5 Manual smoke via `bun run wt:dev` (this worktree's dev slot), walking the spec scenarios: disable a repository from Settings and confirm it leaves the tree, the tray badge drops by its active count, and the Dashboard's summary, breakdown, ships, and streak are unchanged; create a change inside the disabled workspace and confirm no notification fires while the Dashboard total rises; re-enable and confirm the row returns to its original position with live dirty state and branch labels.
- [ ] 7.6 Smoke the terminal and web frontends — `specforge-tui` and a `specforge-serve` build — to confirm both inherit the tree exclusion with no frontend-specific changes.
- [ ] 7.7 Restart the app and confirm the disabled state, display names, and colours all survive; confirm `presentation.json` contains a `disabled` key only for disabled rows.
