# Add a Commit-Graph Rail

## Why

SpecForge already knows a fair amount about git — `git.rs` resolves the repo a workspace belongs to, enumerates worktrees, and tracks each worktree's branch, and `repo_view.rs` uses that to group changes by repository and fan them across worktrees. But it reads **zero commit history**. The tree pane is a snapshot of *current state*: which changes exist, their artifacts, their task progress. What it has no axis for is *time* — when work landed, in what order, by whom, the branch/merge topology, and where each worktree's HEAD actually sits.

For a workflow that fans OpenSpec changes across worktrees, that gap is felt: you can see the change living in a worktree, but not the line of commits that delivered it or how that worktree's branch relates to the others. Today answering "what's the shape of this repo's history right now" means leaving the app for Fork/SourceTree/`git log --graph`.

This change adds a faithful commit graph — the same visual a desktop git client gives — as an always-on rail beside the existing panes, scoped to the repository of whatever is selected in the tree. It is **faithful on purpose**: a plain git graph with branch/merge lanes, refs, and tags, carrying no OpenSpec semantics. The point is to see the repository's real history in the same window as the changes living in it, not to invent a new visualization.

## What Changes

- The main window's two-pane master-detail layout (`SplitPane` wrapping the tree + detail pane) gains a **third pane**: a narrow, resizable, always-on **commit-graph rail** on the far right. It renders the commit graph of the repository that owns the current tree selection.
- A new headless graph layer in `openspec-core` produces the graph: commit data from `git log --all` (hash, parents, author, ISO date, ref decorations, subject), fed into a pure **lane-assignment algorithm** (`graph.rs`) that assigns every commit a column and computes its branch/merge edges. The algorithm lives in the core so it is unit-testable from `cargo test` with no GUI.
- The rail draws the laid-out DAG: a node per commit in its lane, vertical edges where a lane continues and diagonal edges where a branch is created or a merge collapses, ref/tag/HEAD decorations, and a truncated subject. Author, full date, and hash surface on hover; the full set lands in the detail view on click.
- **Lane overflow is handled by compaction first.** Lanes are reclaimed the instant their branch merges, so even `--all` sits at a handful of lanes for most rows; busy merge knots scroll horizontally within the graph gutter, and the rail itself is draggable wider (or collapsible) as the real release valve.
- **Clicking a commit swaps the center detail pane** to a commit-detail view (changed-files list + diff). The tree and the rail both drive the center pane; the most recent selection wins, and a breadcrumb returns to the artifact view. This extends the pane's `RenderTarget` with a new `commit` variant.
- **Live updates** extend the existing per-repo `RepoMonitor` (which already watches `.git/worktrees/`, `.git/config`, and `origin/HEAD`) with a refs watcher over `.git/HEAD`, `.git/refs`, `.git/logs/HEAD`, and `packed-refs`, emitting a graph-changed signal that the rail consumes within the debounce window.

## Capabilities

### New Capabilities

- `commit-graph`: a faithful, read-only git commit-graph rail scoped to the selected repository — graph rendering, lane layout and overflow, ref/tag decorations, commit selection driving the detail pane, the commit-detail view, live updates on ref changes, and graceful degradation when git is absent or the workspace is not a repository.

### Modified Capabilities

- `spec-browser`: the *Master-Detail Layout* requirement changes from a two-pane layout to two primary panes plus an optional third commit-graph rail, and the detail (center) pane gains a non-artifact render target (commit detail) that the rail can drive.

## Impact

- New module `crates/openspec-core/src/graph.rs` — the pure lane-assignment algorithm and the laid-out-commit types crossing the IPC boundary (`#[serde(rename_all = "camelCase")]`). Unit-tested against hand-built DAGs.
- `crates/openspec-core/src/git.rs` — new shell-outs: `commit_log` (`git log --all --pretty=…`), `commit_files` / `commit_diff` (`git show --stat` / `git diff-tree` + per-file unified diff). Same degrade-to-`None`/empty discipline as the existing functions.
- `crates/openspec-core/src/repo_monitor.rs` — a third per-repo watcher over `.git/HEAD`, `.git/refs`, `.git/logs/HEAD`, `packed-refs`; new `CacheEvent` (or a sibling event) signalling the graph should refresh.
- `crates/specforge/src/commands.rs` — `#[tauri::command]` handlers `get_commit_graph`, `get_commit_detail`, `get_commit_diff`, keyed by `repo_id`.
- `crates/specforge/src/events.rs` — forward the graph-changed event to a named Tauri event.
- `src/types.ts` — hand-mirrored `CommitNode` / `LaneLayout` / `CommitDetail` types; the `RenderTarget` union (in `DetailPane.tsx`) gains a `commit` variant.
- `src/components/SplitPane.tsx` / `src/App.tsx` — three-pane composition; `handleSelect` and a new rail-selection handler both write `renderTarget` (last-selection-wins).
- New components `src/components/GraphRail.tsx` and `src/components/CommitDetailView.tsx`; a `useCommitGraph` hook keyed by `repoId` that subscribes to the graph-changed event.
- `src/api.ts` — `invokeLogged` wrappers for the three new commands.
- **Deliberate scope boundaries** (so nobody "fixes" these later as oversights):
  - **Faithful, not OpenSpec-aware.** The graph carries no OpenSpec semantics — no per-change tinting, no commit↔change linking via the `OpenSpec-Id` trailer, no lifecycle markers. An OpenSpec-aware graph was explicitly considered and rejected in favour of a plain git graph. The only OpenSpec coupling is *which repo* the rail shows, inherited from the tree selection.
  - **Read-only.** No checkout, branch create/delete, rebase, cherry-pick, reset, or any mutating git operation is exposed. Visualization only — consistent with v1's read-only `SelfWriteTracker` stance.
  - **Thin git, no `git2`/`gix`.** Everything shells out to the system `git` binary, matching `git.rs` and keeping the Windows cross-compile release pipeline free of a libgit2 C dependency. The cost is more string parsing, which is acceptable.
  - **Rich diff is deferred.** The first increment renders commit detail as `--stat` plus a raw unified diff. A syntax-highlighted, file-tree-navigable diff viewer (as in the reference screenshot) is a fast-follow, not a gate on shipping the rail.
  - **History windowing is bounded explicitly.** The rail loads a capped window of commits with a "load more" affordance rather than the entire history; any cap is surfaced, never silent.
