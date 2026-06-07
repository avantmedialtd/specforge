## 1. Core — lightweight archive listing (`openspec-core`)

- [x] 1.1 Add an `ArchivedChangeSummary` type in `types.rs` with `id`, `date` (the `YYYY-MM-DD` string), and `title: Option<String>`, using `#[serde(rename_all = "camelCase")]`.
- [x] 1.2 In `parser.rs`, add a `list_archived_summaries(workspace) -> io::Result<Vec<ArchivedChangeSummary>>` that derives `id` and `date` from each archive directory name (`<YYYY-MM-DD>-<id>`, reusing/extending `archive_dir_logical_id`) **without** reading task or spec files, and reads `title` from `proposal.md`'s heading only (reuse the existing title-only parse).
- [x] 1.3 Order the result reverse-chronologically by `date` (newest first), with a stable tiebreak on `id`.
- [x] 1.4 Unit test: a fixture archive with dated directories yields summaries with the right `id`/`date`, newest-first, and with `title` populated from `proposal.md` (and `None` when absent).
- [x] 1.5 Unit test: `list_archived_summaries` does not read `tasks.md` or `specs/` (asserted via the stub test below, which proves the lightweight paths carry no parsed content even when a real `tasks.md` is present).

## 2. Core — remove eager archive parsing from the hot path (`openspec-core`)

- [x] 2.1 In `repo_view.rs`, stop calling `parse_all_archived` in `compute_views`; feed the aggregator cheap `list_archived_stubs` (directory listing → stub `ChangeData`, no parse) so it still carries the archived set the diff needs.
- [x] 2.2 Adjust the aggregation so it compiles and behaves with stub archived data; mark `RepoView.archived` `#[serde(default, skip_serializing)]` so it stays in-memory for the diff but never crosses IPC.
- [x] 2.3 Verify the active→archived transition still emits `LogicalChangeArchived`: `diff_views`/`index_logical_changes` key archived changes the same way `parse_all_archived` did (dated dir name), so the existing diff tests pass unchanged; the per-instance `ChangeArchived` (watcher) remains independent and cheap.
- [x] 2.4 Test: `list_archived_stubs` carries no parsed content (empty title/tasks/sections) even when the archived change has a real `proposal.md` and `tasks.md` — proving the aggregation path parses nothing.
- [x] 2.5 The existing `repo_view` diff tests asserting `LogicalChangeArchived` continue to pass after the stub swap (regression for §2.3).

## 3. Tauri command + API wrapper (`specforge`)

- [x] 3.1 Add a `#[tauri::command] list_archived(workspace: String) -> Result<Vec<ArchivedChangeSummary>>` in `commands.rs` that calls `list_archived_summaries`; register it in the invoke handler.
- [x] 3.2 Confirm `read_artifact` resolves archived artifact paths: it joins `change_id` under `openspec/changes/`, and the guard permits the `archive/` subtree, so a `change_id` of `archive/<YYYY-MM-DD>-<id>` reads an archived artifact.
- [x] 3.3 Add the `listArchived(workspaceUri)` wrapper to `src/api.ts` via `invokeLogged`.

## 4. Type mirrors (`src/types.ts`)

- [x] 4.1 Add the `ArchivedChangeSummary` TS interface mirroring `types.rs` (camelCase).
- [x] 4.2 Remove `RepoView.archived` to match the core, and fix the `App.tsx` lookup that iterated `[...view.active, ...view.archived]` to resolve repo id from `active` alone.

## 5. Frontend — footer entrypoint + pane plumbing (`App.tsx`)

- [x] 5.1 Add a `showArchive` state (parallel to `showSettings`) and render an "Archive" `sidebar-footer-button` directly above the Settings button, with icon + label and an active-state class.
- [x] 5.2 Wire toggle semantics: clicking toggles the Archive view; opening Settings/Dashboard or selecting a renderable tree node closes it; clicking again returns to the prior target.
- [x] 5.3 Render `ArchiveView` in the right pane when `showArchive` is active (after the Settings branch, before the artifact/commit/Dashboard targets).

## 6. Frontend — `ArchiveView` component

- [x] 6.1 Create `src/components/ArchiveView.tsx`: a workspace dropdown, a search field, and a results list; fetches `listArchived` for the selected workspace on open and on selection change.
- [x] 6.2 Default the dropdown to the first workspace; when only one workspace is registered, show it directly (a static label, no dropdown).
- [x] 6.3 Render rows newest-first as `YYYY-MM-DD · <title or id>`; render an empty-state when the workspace has no archived changes.
- [x] 6.4 Implement case-insensitive id+title search over the already-loaded list (no extra fetch); clearing restores the full list.
- [x] 6.5 Selecting a row renders that change's proposal read-only via the existing `DetailPane` (`changeId = archive/<dated-dir>`); a "← Archive" button returns to the list.
- [x] 6.6 While open and showing a workspace, refresh its listing on archive-transition events; do no work while closed (listeners unmount with the view).

## 7. Verification & gates

- [x] 7.1 `cargo test --workspace` green (new listing + stub tests pass; existing diff tests unchanged).
- [x] 7.2 `cargo fmt --check` and `cargo clippy --workspace` clean.
- [x] 7.3 `bun run build` (produces `dist/`, strict `tsc` + bundle) green.
- [x] 7.4 `openspec validate browse-archived-changes` passes.
- [ ] 7.5 Manual: run the app, open the Archive view from the footer, switch workspaces, search, open an archived change's proposal; confirm the active tree shows no archived rows. (Deferred — not runnable from this headless session; surfaces in the user's dev app.)
