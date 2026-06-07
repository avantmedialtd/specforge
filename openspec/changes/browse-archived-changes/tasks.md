## 1. Core — lightweight archive listing (`openspec-core`)

- [ ] 1.1 Add an `ArchivedChangeSummary` type in `types.rs` with `id`, `date` (the `YYYY-MM-DD` string), and `title: Option<String>`, using `#[serde(rename_all = "camelCase")]`.
- [ ] 1.2 In `parser.rs`, add a `list_archived_summaries(workspace) -> io::Result<Vec<ArchivedChangeSummary>>` that derives `id` and `date` from each archive directory name (`<YYYY-MM-DD>-<id>`, reusing/extending `archive_dir_logical_id`) **without** reading task or spec files, and reads `title` from `proposal.md`'s heading only (reuse the existing title-only parse).
- [ ] 1.3 Order the result reverse-chronologically by `date` (newest first), with a stable tiebreak on `id`.
- [ ] 1.4 Unit test: a fixture archive with dated directories yields summaries with the right `id`/`date`, newest-first, and with `title` populated from `proposal.md` (and `None` when absent).
- [ ] 1.5 Unit test: `list_archived_summaries` does not read `tasks.md` or `specs/` (assert via a fixture whose archived `tasks.md` would be malformed/oversized if parsed, or by asserting only `proposal.md` is touched).

## 2. Core — remove eager archive parsing from the hot path (`openspec-core`)

- [ ] 2.1 In `repo_view.rs`, stop calling `parse_all_archived` in `compute_views`; remove `archived_changes` from `WorktreeSnapshot` and `archived: Vec<LogicalChange>` from `RepoView` (or reduce to a cheap archived-id set only if §2.3 shows the diff needs it).
- [ ] 2.2 Adjust `aggregate` / `diff_views` and any consumers so they compile and behave without the parsed `RepoView.archived`.
- [ ] 2.3 Verify the active→archived transition still emits `LogicalChangeArchived`: confirm `diff_views` derives it from the active-set delta (and the per-instance `ChangeArchived` from the watcher's cheap archived-id membership test) without needing parsed archived content. Keep a directory-listing-based archived-id set in the aggregation only if the diff actually requires it.
- [ ] 2.4 Test: a watcher re-aggregation triggered by an active-change edit performs no archive parse (assert `compute_views` does not read `openspec/changes/archive/`) — covers *On-Demand, Off-Hot-Path Loading → Watcher batch does not parse the archive*.
- [ ] 2.5 Test: archiving a change's last active instance still produces `LogicalChangeArchived` (regression for §2.3).

## 3. Tauri command + API wrapper (`specforge`)

- [ ] 3.1 Add a `#[tauri::command] list_archived(workspace: String) -> Result<Vec<ArchivedChangeSummary>>` in `commands.rs` that calls `list_archived_summaries` for the registered workspace; register it in the invoke handler.
- [ ] 3.2 Confirm `read_artifact` already resolves archived artifact paths under `openspec/changes/archive/…` (they are inside `openspec/changes/`); add a focused test reading an archived `proposal.md` through `read_artifact`.
- [ ] 3.3 Add the `listArchived(workspaceUri)` wrapper to `src/api.ts` via `invokeLogged`.

## 4. Type mirrors (`src/types.ts`)

- [ ] 4.1 Add the `ArchivedChangeSummary` TS interface mirroring `types.rs` (camelCase).
- [ ] 4.2 Remove `RepoView.archived` (and adjust `ChangeInstance.isArchivedHere` if it becomes unused) to match the core, and fix the `App.tsx` lookup that iterates `[...view.active, ...view.archived]` to resolve repo id from `active` alone (or a path→repo map).

## 5. Frontend — footer entrypoint + pane plumbing (`App.tsx`)

- [ ] 5.1 Add a `showArchive` state (parallel to `showSettings`) and a `kind:"archive"` detail target; render an "Archive" `sidebar-footer-button` directly above the Settings button, with icon + label and an active-state class.
- [ ] 5.2 Wire toggle semantics: clicking toggles the Archive view; opening Settings/Dashboard or selecting a renderable tree node closes it; clicking again returns to the prior target — mirrors *Archive Entrypoint in Sidebar Footer*.
- [ ] 5.3 Render the new `ArchiveView` in the right pane when `showArchive` is active (precedence over artifact/commit/Dashboard, below Settings per existing modal-pane ordering).

## 6. Frontend — `ArchiveView` component

- [ ] 6.1 Create `src/components/ArchiveView.tsx`: a workspace dropdown (display name → basename fallback), a search field, and a results list; fetches `listArchived` for the selected workspace on open and on selection change (*On-Demand, Off-Hot-Path Loading*).
- [ ] 6.2 Default the dropdown to a sensible workspace; when only one workspace is registered, show it directly with the dropdown disabled/non-interactive (*Workspace Scoping via Dropdown*).
- [ ] 6.3 Render rows newest-first as `YYYY-MM-DD · <title or id>`; render an empty-state when the workspace has no archived changes (*Archive View*, *Newest-First Ordering and Date Labels*).
- [ ] 6.4 Implement case-insensitive id+title search over the already-loaded list (no extra fetch); clearing restores the full list (*Search Within the Selected Workspace's Archive*).
- [ ] 6.5 Selecting a row renders that change's artifact read-only via the existing markdown path, reading from the archive directory (*Read-Only Artifact Navigation*); provide a way back to the list.
- [ ] 6.6 While open and showing a workspace, refresh its listing on archive-transition events for that workspace; do no work while closed (*Live Refresh of the Open Archive View*).

## 7. Verification & gates

- [ ] 7.1 `cargo test --workspace` green (new listing + hot-path + diff regression tests pass).
- [ ] 7.2 `cargo fmt --check` and `cargo clippy` clean.
- [ ] 7.3 `bun run build` (produces `dist/`, strict `tsc` + bundle) green.
- [ ] 7.4 `openspec validate browse-archived-changes` passes.
- [ ] 7.5 Manual: run the app, open the Archive view from the footer, switch workspaces, search, open an archived change's proposal; confirm the active tree is unchanged and shows no archived rows.
