## 1. Core: commit data extraction (`git.rs`)

- [x] 1.1 Add `commit_log(repo_id, limit)` — shell `git --git-dir <common> log --all --date-order --pretty=format:'<sep-delimited %H %P %an %aI %D %s>'`, capped at `limit`; parse into a `Vec<RawCommit>` (hash, parents, author, ISO date, decorations, subject). Degrade to empty vec on git error, matching the existing functions
- [x] 1.2 Parse the `%D` decoration field into structured refs (local branch / remote branch / tag / HEAD pointer), so the renderer gets typed decorations rather than a raw string
- [x] 1.3 Add `commit_files(repo_id, sha)` — `git diff-tree --no-commit-id --name-status -r <sha>` (and `--numstat` for +/− counts) parsed into a changed-files list
- [x] 1.4 Add `commit_diff(repo_id, sha, path)` — `git show <sha> -- <path>` (or `git diff-tree -p`) returning the raw unified diff for one file
- [x] 1.5 Unit tests for each, building a small repo with `tempfile` + `git` (mirroring the existing `git.rs` test harness): linear history, a branch+merge, a tag, and a renamed file

## 2. Core: lane-assignment algorithm (`graph.rs`)

- [x] 2.1 Define the laid-out types: `LaidOutCommit { commit_id, column, row, edges: Vec<EdgeSegment>, refs }` and the IPC-facing graph payload, all `#[serde(rename_all = "camelCase")]`
- [x] 2.2 Implement the pure layout function: sweep newest→oldest maintaining `lanes: Vec<Option<CommitId>>`; assign each commit its column (reserved-by-child else new lane), continue the first parent down the same lane, allocate lanes for additional parents (merges), and collapse fork lanes into the commit
- [x] 2.3 Emit per-row `EdgeSegment`s (lanes passing straight through, bends in/out at the commit) so the renderer draws geometry without re-deriving topology
- [x] 2.4 Implement lane compaction: reclaim a lane as soon as its branch merges, so visible lane count stays small under `--all`
- [x] 2.5 Unit tests asserting column + edge assignments for: linear, single fork+merge, octopus (3+ parent) merge, criss-cross merges, and a long-dormant branch reactivated late (the date-order gap case)

## 3. Core: live updates (`repo_monitor.rs`)

- [x] 3.1 Add a third per-repo watcher over `.git/HEAD`, `.git/refs`, `.git/logs/HEAD`, and `packed-refs` (best-effort install, like the existing meta/config watchers), debounced
- [x] 3.2 Emit a graph-changed signal on debounced ref batches (a new `CacheEvent` variant or a sibling event on the same broadcast channel) carrying the affected `repo_id`
- [x] 3.3 Confirm teardown: the new watcher and task are disposed on `RepoMonitor::drop` alongside the existing two

## 4. Shell: commands + events

- [x] 4.1 Add `#[tauri::command]` `get_commit_graph(repo_id, limit)` → runs `commit_log` + `graph.rs` layout, returns the IPC graph payload
- [x] 4.2 Add `get_commit_detail(repo_id, sha)` (changed files + counts) and `get_commit_diff(repo_id, sha, path)` (one file's unified diff)
- [x] 4.3 In `events.rs`, bridge the graph-changed `CacheEvent` to a named Tauri event (constant name shared with the frontend), consistent with the existing `cache-updated` / `change-added` / `change-archived` bridges
- [x] 4.4 Register the new commands in the shell's invoke handler

## 5. Frontend: types + API

- [x] 5.1 Mirror the new core types in `src/types.ts` by hand (`CommitNode`/`LaidOutCommit`, `LaneLayout`/`EdgeSegment`, `CommitDetail`, ref decorations) — keep camelCase parity with the serde structs
- [x] 5.2 Extend the `RenderTarget` union (in `DetailPane.tsx`) with `{ kind: "commit", repoId, sha }`
- [x] 5.3 Add `invokeLogged` wrappers in `src/api.ts` for `get_commit_graph`, `get_commit_detail`, `get_commit_diff`
- [x] 5.4 Add a `useCommitGraph(repoId)` hook that fetches the graph window and refetches on the graph-changed event (mirroring `useWorkspaces`' event wiring)

## 6. Frontend: three-pane layout

- [x] 6.1 Extend `SplitPane` (or compose a second instance) to support a third, resizable, far-right pane; persist the rail width like the existing divider position
- [x] 6.2 In `App.tsx`, render the `GraphRail` as the third pane, targeted at the repo of the current tree selection (`repo_id`); render a placeholder when the selection is a flat/non-git workspace or nothing is selected
- [x] 6.3 Wire rail commit-click to set `renderTarget` to the `commit` variant; ensure tree selection and rail selection both write `renderTarget` with last-selection-wins, each keeping its own highlight

## 7. Frontend: graph rail rendering (`GraphRail.tsx`)

- [x] 7.1 Render the laid-out DAG (SVG or canvas): a node per commit in its column, vertical edges for continuing lanes, diagonal edges for branch/merge, from the layout's `EdgeSegment`s
- [x] 7.2 Render ref/tag/HEAD decorations and a truncated commit subject per row; show author + full date + hash on hover
- [x] 7.3 Handle lane overflow: compaction keeps the common case narrow; add horizontal scroll inside the graph gutter for spikes without scrolling the subject away
- [x] 7.4 Add a "load more" affordance that grows the window; surface any active cap explicitly (no silent truncation)
- [x] 7.5 Empty/degenerate states: not-a-repo, single commit, detached HEAD, git missing

## 8. Frontend: commit-detail view (`CommitDetailView.tsx`)

- [x] 8.1 Render the commit-detail center-pane variant: metadata header (sha, author, date, full message), the changed-files list with +/− counts, and a raw unified diff (stage 2 scope — no syntax highlighting yet)
- [x] 8.2 Add the breadcrumb (`<sha> · select an artifact to return`) so the swap back to artifacts is discoverable

## 9. Spec sync (applied at archive time via `openspec archive`)

- [ ] 9.1 Apply the `commit-graph` delta (new capability) from `openspec/changes/add-commit-graph-rail/specs/commit-graph/spec.md`
- [ ] 9.2 Apply the `spec-browser` delta from `openspec/changes/add-commit-graph-rail/specs/spec-browser/spec.md` (modify *Master-Detail Layout* for the three-pane layout + commit render target)

## 10. Manual verification

- [ ] 10.1 Run `bun tauri dev` against a registered git workspace; confirm the rail shows the repo's `--all` graph with branch/merge lanes, refs, and tags matching `git log --graph --all`
- [ ] 10.2 Confirm lane compaction keeps the narrow rail readable; drag the rail wider and confirm topology expands; confirm a busy merge knot scrolls horizontally without losing the subject column
- [ ] 10.3 Click a commit; confirm the center pane swaps to the commit detail (files + diff), the breadcrumb appears, and clicking a tree artifact restores the markdown
- [ ] 10.4 Make a commit / move a branch on disk; confirm the rail refreshes within the debounce window without user action
- [ ] 10.5 Select a non-git (flat) workspace; confirm the rail shows the empty/placeholder state and the app is otherwise unaffected
- [ ] 10.6 Rename `git` off PATH (or point at a non-repo); confirm the rail degrades to empty and nothing else breaks

## 11. Build check

- [x] 11.1 Run `bun run build` and confirm `tsc --noEmit` + Vite build succeed under `noUnusedLocals` / `noUnusedParameters` (the new `RenderTarget` variant and hook must type-check)
- [x] 11.2 Run `cargo test` and confirm the new `graph.rs` and `git.rs` tests pass alongside the existing suite
