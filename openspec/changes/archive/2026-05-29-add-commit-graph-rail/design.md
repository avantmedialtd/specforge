# Design — Add a Commit-Graph Rail

## Decisions

### Faithful git graph, not an OpenSpec-aware one

The rail renders a plain git commit graph with no OpenSpec semantics. Commits are not tinted by their `OpenSpec-Id` trailer, the graph is not filtered by the selected change, and archive commits carry no special "shipped" marker.

- **Why:** this was an explicit fork during exploration. An OpenSpec-aware graph (colour each commit by the change it delivered, highlight the commits for the selected change) is the version only SpecForge could build, but it was rejected in favour of the thing the user actually asked for — a faithful Fork/SourceTree-style graph.
- **Accepted cost:** the rail re-implements a generic capability that standalone git clients already provide, with the differentiation coming only from *where* it lives (in-app, scoped to registered repos, beside the changes) rather than from the graph itself. The `OpenSpec-Id` trailer is a real, machine-readable join key that this design deliberately leaves on the table; an OpenSpec-aware overlay remains an obvious future option (see *Open / deferred*).
- The only OpenSpec coupling retained is **scope**: the rail shows the repository that owns the current tree selection.

### Always-on third rail (resizable), scoped to the selected repo

The rail is a permanently visible third pane on the far right, not a mode you toggle into. Placement was chosen over two alternatives: a "third mode" of the existing right pane (swaps in like Settings) and a dedicated full-window git view.

- **Consequence — "always-on" makes live updates core, not optional.** A toggled mode could lazily `git log` on entry; an always-visible rail must stay live for the whole session, so the refs watcher (below) is load-bearing rather than a nicety.
- **Scope:** the rail follows the tree selection's repository (`repo_id`). When the selection is a non-git (flat) workspace, or nothing is selected, the rail shows an empty/placeholder state. With multiple registered repos the rail re-targets as selection moves between them.
- **Resizable** via the existing `SplitPane` machinery (extended to three panes). The rail is narrow by default and can be dragged wider — this is the primary release valve for the width constraint below.

### `--all` ref set, made readable by lane compaction

The graph is built from `git log --all` — every local and remote branch and tag, the full DAG — chosen over a worktree-scoped default and a `--first-parent` linear view.

- **The tension this creates:** a faithful `--all` graph in a ~240px rail can overflow horizontally when many branches are concurrently alive. This was accepted knowingly.
- **How it's made to work, in priority order:**
  1. **Lane compaction.** A lane is reclaimed the instant its branch merges, so the *visible* lane count at any vertical position is far smaller than the total branch count. Most rows sit at ~3–5 lanes even under `--all`; width spikes only at busy merge knots.
  2. **Resizable rail** — drag it wider to study topology when the narrow default is too tight.
  3. **Horizontal scroll inside the graph gutter** — last resort for pathological knots, so a spike never pushes the subject column off-screen permanently.
- A `--first-parent` "simplify" toggle is a cheap future addition if `--all` still reads as noisy; it is not in the first increment.

### The lane-assignment algorithm (the hard part) lives in the core

Lane layout is the one part with no off-the-shelf answer for an interactive UI, so it lives in `openspec-core/src/graph.rs` as a **pure function** — DAG in, laid-out commits out — and is unit-tested from `cargo test` with no GUI, per the project's "watchers/parsers/git logic belong in the core" rule.

The algorithm, sweeping newest → oldest over commits in date order, maintaining `lanes: Vec<Option<CommitId>>` where each slot holds the commit a lane is currently waiting to draw:

```
for each commit C:
  1. lane(C) = first slot reserved for C by a child; else allocate a new lane (a tip)
  2. C.column = that lane index
  3. replace the slot with C's FIRST parent        → lane continues straight down
  4. each ADDITIONAL parent (a merge) → reserve a free lane, emit a diagonal edge
  5. other lanes also waiting for C (a fork) → collapse them into C's lane

vertical edge  = a lane continues       diagonal edge = branch created / merge collapsed
```

- **Edges, not just columns.** The layout emits per-row edge segments (which lanes pass straight through, which bend in/out at this commit) so the renderer is dumb — it draws what the layout computed, it does not reason about topology.
- **Tests write themselves:** feed a known DAG (linear, a fork+merge, a multi-merge octopus, criss-cross merges) and assert the column assignments and edge sets. This is the bulk of the core test surface for the feature.

### Two inputs, one center pane — "last selection wins"

Clicking a commit in the rail swaps the center detail pane to a commit-detail view; clicking a change/artifact in the tree swaps it back to artifact markdown. Both the tree and the rail write the same `renderTarget` state.

- **Model:** the center pane renders whatever was *last* selected from either input. The tree and the rail each keep their own selection highlight. No modal, no explicit "back" button: clicking a tree artifact restores markdown.
- **Discoverability:** a thin breadcrumb in the commit-detail view (`<sha> · select an artifact to return`) covers the one-way-door feeling of the swap.
- **Type impact:** `RenderTarget` (defined in `DetailPane.tsx`) becomes a discriminated union gaining `{ kind: "commit", repoId, sha }` alongside the existing artifact variants. This was chosen over a flyout/popover (a full diff is cramped in a popover; new floating-panel pattern) and over expanding the rail itself into a graph+detail panel (heavier interaction, occludes the tree).

### Live updates extend `RepoMonitor`, not new infrastructure

`RepoMonitor` already installs per-repo `notify` watchers over `.git/worktrees/` (worktree reconciliation) and `.git/config` + `.git/refs/remotes/origin/HEAD` (default-branch refresh). The graph adds a **third watcher** on the same pattern, over `.git/HEAD`, `.git/refs`, `.git/logs/HEAD`, and `packed-refs`, debounced, emitting a graph-changed event.

- **Why not the main watcher:** the existing `WatcherManager` is scoped to each workspace's `openspec/` subtree by design and must stay that way. Ref watching belongs with the repo-level monitor that already reaches into `.git/`.
- **Cheap refresh:** on a graph-changed event the frontend re-fetches the current window via `git log`; with windowing (below) this is bounded work, so re-running on each debounced ref batch is affordable.

### Shell out to git; window the history

All git access is `Command::new("git")`, degrading to `None`/empty on failure exactly like the existing `git.rs` functions — no `git2`/`gix`, keeping the Windows cross-compile pipeline free of a libgit2 C dependency.

- **Windowing:** the rail requests a capped window of commits (e.g. the most recent N across `--all`) with a "load more" affordance, rather than the whole history. The app reads everything else eagerly today; the graph is the first surface that must paginate, because repos can hold 10⁴–10⁵ commits. Any cap that bounds what's shown is surfaced in the UI, never silent.

## Staging

The placement decision ("swap the center pane") fixes *where* commit detail goes, not *how rich* it is. The work is staged so the rail is not gated on building a diff viewer:

1. **Graph rail** — `git log --all` → `graph.rs` layout → `GraphRail` rendering, refs/tags, hover, windowing, live refresh. This is the thing asked for.
2. **Basic commit detail** — clicking a commit swaps the center pane to `--stat` + a raw unified diff.
3. **Rich diff (fast-follow, deferred)** — file-tree navigation, per-file collapse, syntax highlighting, +/- gutters, as in the reference screenshot.

## Open / deferred

- **OpenSpec-aware overlay.** The `OpenSpec-Id` trailer + archive-commit detection could later tint commits by change or highlight the commits delivering the selected change — an optional overlay on top of the faithful graph, not a replacement. Explicitly out of this change.
- **`--first-parent` simplify toggle** for taming `--all` noise.
- **Rich diff viewer** (stage 3 above).
- **Multi-repo behaviour when nothing is selected** — whether the rail shows the most-recently-active repo, the first registered repo, or stays empty. First increment: empty until a git-backed node is selected.
- **Interactive git operations** (checkout, branch ops, etc.) — out of scope by the read-only boundary; would be a separate proposal with its own safety design.

## What does not change

- The headless-core / Tauri-shell split: graph data, lane layout, and ref watching live in `openspec-core`; the shell only wraps them in commands and events.
- The existing `WatcherManager` and its `openspec/`-subtree scope.
- The tree pane, its selection contract, and the existing artifact `RenderTarget` variants — they are extended, not altered.
- The read-only posture of the app and the `SelfWriteTracker` pipeline.
