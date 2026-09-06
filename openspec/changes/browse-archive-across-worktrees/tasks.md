## 1. Core: logical identity for archived changes

- [x] 1.1 In `crates/openspec-core/src/parser.rs`, confirm `archive_dir_logical_id` / `archive_dir_date` are the single place a dated archive directory name is split, and export whatever the union needs; add a test pinning that the date strip is applied exactly once, so an id that itself begins with a date-shaped prefix round-trips (`archive-browser`: *Union Archive Listing Across a Repository's Worktrees*).
- [x] 1.2 In `crates/openspec-core/src/repo_view.rs`, key **archived** instances in `build_repo_view`'s `by_name` map on the bare logical id rather than the raw dated directory name, so an active instance and an archived one of the same change land in one `LogicalChange` (`spec-browser`: *Per-Instance Divergence Label*). Keep the dated name on the instance — it is what addresses a read.
- [x] 1.3 Re-point the divergence fixtures in `repo_view.rs` (`build_workspace` and every test that writes `archive/foo`) at **dated** directories, so `change_archived_on_default_and_active_on_branch_gets_stale_label` stops passing for the wrong reason, and add a case asserting `[stale]` fires for `archive/2026-09-05-add-thing/` against active `add-thing` (`spec-browser`: *Stale label fires against a dated archive directory*).
- [x] 1.4 Audit the other consumers of the dated-name key and update them together: `git::change_lifecycle` in `crates/openspec-core/src/git.rs` (which splits one change into two `ChangeLifecycle` rows) and the ships join in `crates/openspec-core/src/dashboard.rs` that documents working around that split. Fix or explicitly re-justify each; do not leave two identity schemes.
- [x] 1.5 Add the union row and copy shapes to `crates/openspec-core/src/types.rs` (`#[serde(rename_all = "camelCase")]`): a row carrying the bare logical id, the display date, the title, and its copies, each copy carrying its worktree path and its archive directory name.
- [x] 1.6 Implement the pure grouping function in `openspec-core` — per-worktree listings in, de-duplicated rows out — with newest-date-wins for the row's date and a deterministic total order (`archive-browser`: *Newest-First Ordering and Date Labels*).
- [x] 1.7 Write adversarial tests for 1.6 that the mutation gate cannot pass vacuously: a tie on archive date resolved by the stable tie-break, a row collapsing `2026-06-04-x` and `2026-06-05-x` reporting `2026-06-05`, a legacy un-dated directory collapsing with its dated twin, and a row whose copies are all un-dated reporting no date.

## 2. App service: the repository-scoped union

- [x] 2.1 In `crates/openspec-app/src/service.rs`, add the repo-scoped union operation: resolve the repository's tracked worktrees from the registry (user-registered **and** discovered), call the existing per-workspace listing on each, and group via 1.6 (`archive-browser`: *Union Archive Listing Across a Repository's Worktrees*).
- [x] 2.2 Authorize it through the existing `ensure_registered_repo`, and add a test that an unregistered repository identifier is refused with nothing enumerated (`archive-browser`: *Union listing for an unregistered repository is refused*).
- [x] 2.3 Extend the archive-confinement test to cover a registry-**discovered** worktree: `list_archived` and `archived_artifact_status` must succeed for it, pinning the behaviour the union depends on (`archive-browser`: *Archive listing for a registry-discovered worktree succeeds*).
- [x] 2.4 Run the union off the async runtime the way the other filesystem-walking operations do, and confirm by test that nothing it does runs during watcher aggregation (`archive-browser`: *On-Demand, Off-Hot-Path Loading*).

## 3. IPC: register the new command in all four places

- [x] 3.1 Add the `#[tauri::command]` handler in `crates/specforge/src/commands.rs` — deserialize args and call `AppService`, nothing more.
- [x] 3.2 Add it to the `tauri::generate_handler![…]` list in `crates/specforge/src/lib.rs`.
- [x] 3.3 Add the match arm to the `/api/invoke` table in `crates/specforge-web/src/dispatch.rs`. Skipping this compiles and passes `tsc`, then fails at runtime in the browser with `unknown command` — and the served web UI is the verification path.
- [x] 3.4 Add the wrapper in `src/api.ts`.
- [x] 3.5 Hand-mirror the union row and copy types from 1.5 into `src/types.ts`; there is no codegen, so both sides move in this commit.
- [x] 3.6 **Unplanned, found by the runtime smoke.** `#[serde(rename_all)]` on an *enum* renames its variants, not the fields of a struct variant — so `ArchiveScope::Repo { repo_id }` stayed snake_case while `src/api.ts` sent `repoId`, and every repository-scoped listing failed at the wire with `missing field repo_id` while `cargo test`, `tsc`, `bun test` and the mutation gate all stayed green. Add `rename_all_fields = "camelCase"`, and add wire-shape tests in `crates/openspec-core/src/types.rs` that assert against the literal JSON `src/api.ts` sends and reads, in both directions.

## 4. Frontend: union listing and copy selection

- [x] 4.1 In `src/components/ArchiveView.tsx`, change the scope selector's option pool from `RegisteredWorkspace[]` to top-level rows (repository groups and flat workspaces) and fetch the union for the selected scope (`archive-browser`: *Archive View*).
- [x] 4.2 Split the state: keep the listing-scope variable driving the listing fetch, and add a **separate** per-open `activeCopy` variable driving the render target. Do not reuse `selectedUri` — its `onChange` clears the open change and refetches the listing, so a copy switch through it would close the change being read (design D6).
- [x] 4.3 Feed `activeCopy` into `ArtifactRenderTarget.workspace` and the change's archive directory name into its `changeId`, so the reader renders the selected copy (`archive-browser`: *Read-Only Artifact Navigation*).
- [x] 4.4 Render the copy control: a chooser when the row has several copies, a plain non-interactive label when it has one. Label by workspace display name falling back to worktree basename — never by branch (`archive-browser`: *Copy Selection Within an Opened Archived Change*).
- [x] 4.5 Re-fetch `archivedArtifactStatus` against the newly selected copy when `activeCopy` changes, since two copies need not hold the same artifacts (`archive-browser`: *Switching copy re-determines the offered artifacts*).
- [x] 4.6 Make the row key unique across worktrees — the current key is the reconstructed dated directory name, which is no longer unique within a row's scope.
- [x] 4.7 Widen the live-refresh effect so an archive transition in any tracked worktree of the open scope refreshes the listing without closing the open change (`archive-browser`: *Archival in a sibling worktree refreshes the listing*).
- [x] 4.8 Point the search filter at the union rows, matching identifier and title as before; confirm no additional filesystem read is triggered by filtering.
- [x] 4.9 Add the copy-control styles to `src/App.css`, following the existing archive header and tab treatments.

## 5. Frontend: the today's-ships deep link

- [x] 5.1 In `src/routing/resolve.ts`, rework `worktreeForHint` so a ship's hint resolves against the repository's tracked worktrees rather than only active instances and the user-registered listing — the two pools that both miss an auto-discovered worktree holding an archived change (`dashboard`: *Selecting a ship archived in an auto-discovered worktree*).
- [x] 5.2 Remove `ArchiveView`'s selection-validity fallback that snaps an unrecognised workspace URI to the first registered workspace, which discards a valid deep link; the union makes the change present regardless, so the hint now only chooses which copy opens first.
- [x] 5.3 Fix `src/workspaceRows.test.ts`'s "the destination is a workspace the Archive browser can actually select" case, whose comment asserts "a ship's `worktreePath` is always a registered folder" — false for a discovered worktree, and the reason the test passes today is that its fixture registers the feature worktree. Rewrite it around a discovered worktree so it fails against the current behaviour.
- [x] 5.4 Add a resolve test for a ship whose `worktreePath` is a discovered worktree absent from the registered listing, asserting the archive address keeps its pre-selection.

## 6. Verification

- [x] 6.1 `bun install && bun run build` once in this worktree before any `cargo` run — `dist/` is gitignored and both `generate_context!` and specforge-web's `RustEmbed` need it, and a stale `dist/` makes the debug web app serve a pre-change UI.
- [x] 6.2 `cargo fmt --check` and workspace `cargo clippy -- -D warnings`; both gate CI.
- [x] 6.3 `cargo test` (workspace). A red baseline poisons the mutation run, so this must be green before 6.4.
- [x] 6.4 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Survivors on the de-duplication key, the row date choice, or the copy ordering mean 1.7 is not adversarial enough — add the assertion rather than excluding the mutant, and never reach for `--baseline=skip`.
- [x] 6.5 `bun run build` (strict `tsc` with `noUnusedLocals`/`noUnusedParameters`, then bundle).
- [x] 6.6 Manual smoke — start the app yourself, do not ask the user. Create two worktrees of this repo, archive a change in one, and walk the scenarios: the change appears in the union scoped to the repository; the copy control is a label for it; a change present in both worktrees shows a chooser and switching re-renders without closing; no branch name appears anywhere in the reader.
- [x] 6.7 Manual smoke of the deep link: with a change archived only in a discovered worktree, click its today's-ships entry and confirm the Archive browser opens with that change pre-selected rather than falling back to the main worktree.
- [x] 6.8 Confirm the tree pane now shows `[stale]` for a change archived on the default branch under a dated directory while still active on a feature branch — the case that could not fire before.
- [x] 6.9 Verify in the browser path (`specforge-serve` plus `bun run dev`) as well as the Tauri shell, so a missing `dispatch.rs` arm cannot slip through.
