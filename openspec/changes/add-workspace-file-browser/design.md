# Design — Add Workspace File Browser

## Context

Clicking a top-level row in the sidebar (a Repo group or a flat workspace node) is deliberately inert today: `App.tsx`'s `handleSelect` returns early for the `repo` / `workspace` selection kinds, and the `spec-browser` capability's *Deferred Interaction Nodes* requirement reserves those clicks "for later UX work". The center pane is driven by the `RenderTarget` union (`artifact | commit | dashboard`), so a new surface slots in as a fourth variant without touching the tree-selection contract.

Everything the browser needs already exists in some form:

- `MarkdownView` renders markdown (with mermaid) for artifacts and is target-agnostic.
- `read_artifact` establishes the canonicalise-then-prefix-check read-guard pattern, currently scoped to `openspec/changes/`.
- `git.rs::git_command()` is the single chokepoint for git invocations, including the Windows/WSL routing through `wsl.exe` (filesystem access to `\\wsl.localhost` shares is slow; git-inside-the-VM is fast).
- `ArchiveView` is the precedent for a center-pane surface that loads its own data on mount and never touches the watcher hot path.
- `specforge-web` serves the same React bundle and dispatches commands through an explicit table in `dispatch.rs`.

The app is a read-only viewer (v1); the browser inherits that.

## Goals / Non-Goals

**Goals:**

- Turn the dead top-level click into a read-only markdown browser for the workspace.
- Enumeration respects `.gitignore` for git repos, returns only `.md` files, and is cheap — including on WSL workspaces.
- Cover both Repo groups and flat (non-git) workspaces.
- Same feature over the web transport (shared bundle + dispatch entries).
- No new crate dependencies, no new file watchers.

**Non-Goals:**

- Editing files (the app is a read-only viewer).
- Viewing non-markdown files, full-text content search, or Quick Open.
- Watching the whole repository for live updates (freshness is pull-based).
- A worktree picker — a Repo group browses its **main worktree** in v1.
- Browser UI state persistence across sessions (expansion/filter state is ephemeral).
- A TUI equivalent.

## Decisions

### 1. Enumerate from the git index, not by walking the filesystem

For a git browse root, the list comes from:

```
git ls-files --cached --others --exclude-standard -z -- ':(icase)*.md'
```

run at the browse root through the existing `git_command` chokepoint.

- **Correctness**: `--exclude-standard` is git's own ignore semantics — nested `.gitignore`s, negations, `info/exclude`, global excludes — with nothing reimplemented. `--others` keeps untracked-but-not-ignored drafts visible; ignored untracked files are excluded.
- **Performance**: git reads its index rather than the directory tree, so ignored directories (`node_modules/`, `target/`, …) are never visited at all.
- **WSL**: the chokepoint routes the call through `wsl.exe` on Windows, so enumeration runs inside the VM instead of stat-ing over the 9P share.
- **Mechanics**: `-z` NUL-delimiting (no quoting issues), `:(icase)` catches `README.MD`, results deduped through a `BTreeSet` (unmerged index entries can repeat a path) which also yields sorted output. Paths arrive repo-relative with forward slashes on every platform.

**Alternatives considered**: the `ignore` crate (new dependency, walks the real filesystem — pathological over 9P — and duplicates semantics git already owns); lazy per-directory listing (one round trip per folder, plus the "does this subtree contain any `.md`?" pruning problem the flat list dissolves for free).

### 2. Flat (non-git) workspaces use a small bounded walk

`.gitignore` semantics don't exist outside a repository, so flat roots get a std-lib recursive walk that: skips entries whose name starts with `.`, never follows directory symlinks (cycle/escape safety), skips a short junk-name list (`node_modules`, `target`, `dist`, `build`, `out`, `vendor`, `__pycache__`), caps depth defensively, and keeps only `.md` files (case-insensitive). Output is normalised to forward-slash relative paths so the frontend sees one shape. The heuristic list is acceptable because it only applies to non-git roots, which are rare and small in practice.

### 3. One-shot flat list; the folder tree is derived client-side

The command returns `Vec<String>` (relative paths). The frontend derives the folder hierarchy in a `useMemo`: expanding a folder costs zero IPC, and a directory with no markdown anywhere beneath it can never appear because folders only materialise from file paths. Even markdown-heavy repos yield a few thousand short strings — trivial over IPC. A server-shaped tree was rejected as more Rust surface with no benefit.

### 4. A `files` render target and an ArchiveView-style center surface

`RenderTarget` gains `{ kind: "files"; root: string; label: string }`. `handleSelect` resolves the browse root at click time: a `repo` selection finds its `RepoView` and uses `mainWorktree` (label: `displayName ?? name`); a `workspace` selection uses the workspace URI directly. The commit rail keeps re-scoping exactly as today (that code path is untouched). A new `FileBrowserView` renders two regions — folder tree and rendered markdown via `MarkdownView` — and owns its own fetch/expansion/filter/selection state, mirroring `ArchiveView`'s lifecycle. Sidebar nesting of files was rejected: it would pollute the change-shaped tree contract, add thousands of always-rendered rows, and go stale without a watcher.

### 5. Two independent guards, sibling to `read_artifact`

**Authorization (which roots).** Both `list_markdown_files` and `read_workspace_file` first run the caller-supplied root through `ensure_browse_root`, which accepts it only when it is a registered workspace or a path inside a registered repository, and returns the canonical root used for the subsequent resolution. This is non-negotiable: the shipped `authorize-command-paths` change (archived 2026-07-01) established that every path-taking read authorizes against the registry "at the shared application boundary so it holds for every frontend and transport", precisely because `/api/invoke` may be reachable over Tailscale. Without it, root-prefix checking is vacuous as an authorization boundary — the caller picks the root, so an arbitrary path would yield filesystem enumeration plus arbitrary `.md` read. The repo case must accept a repository's main worktree when only a *linked* worktree is registered, since that is exactly what a Repo group row browses. Refusals report one uniform message; which membership test failed is an implementation detail.

**Path guard (where within a root).** Then, as before: reject absolute paths and `..` components up front, join to the canonical root, canonicalise (`paths::canonicalize`, dunce-backed), require the result to stay under the root (this also rejects symlinks escaping the workspace), require a `.md` extension case-insensitively, and cap content at 5 MiB (defensive; markdown that size would drown the renderer anyway). Failures are readable strings surfaced in the pane.

Membership in the previously enumerated list is deliberately **not** required — authorization plus root-prefix plus extension is the boundary, and a listing-membership check would force a server-side cache for no added safety.

### 6. Freshness is pulled, not pushed

The listing is fetched when the browser opens or its root changes, and on a manual refresh control. No repo-wide watcher is registered — that cost is exactly what the `openspec/`-scoped watcher design avoids, and enumeration is a single cheap git spawn. If staleness ever grates, the existing `graph-changed` event (a commit landed) is a natural future trigger; explicitly out of scope now.

### 7. Blocking work off the async runtime

Enumeration and reads run via `spawn_blocking` inside `AppService`, matching the commit-graph pattern ("runs the blocking git calls off the async runtime").

### 8. Presentation details

Folders sort before files, both case-insensitively. A filter input (substring match on the relative path, case-insensitive) mirrors the Archive view's filter; matches render with their ancestor folders revealed. Styling reuses existing tokens in `App.css`.

## Risks / Trade-offs

- [`ls-files --cached` lists files staged or tracked but deleted on disk] → the read fails with a friendly error in the preview pane; refresh after the state settles. Accepted.
- [Tracked-but-gitignore-matched files appear] → the index is the contract; a tracked doc is a real doc. Accepted as correct git semantics.
- [Very large markdown sets could bloat the client tree] → paths-only payload plus collapsed-by-default folders keeps DOM size bounded; virtualization deferred until a real repo proves the need.
- [Junk-dir list for flat roots is heuristic] → applies only to non-git roots; depth cap bounds the damage.
- [Submodule markdown is invisible (`ls-files` stops at the gitlink)] → accepted for v1; a submodule is its own repo and can be registered separately.
- [git missing / repo broken] → enumeration returns an error; the pane shows a non-crashing error state, mirroring the commit graph's degrade-to-empty behaviour.
- [File edited between listing and read] → the read is authoritative; a stale listing is harmless.

## Open Questions

None blocking. Two deliberate deferrals recorded here so they aren't re-litigated: the extension set is exactly `.md` (adding `.mdx`/`.markdown` is a one-line pathspec change later), and multi-worktree browsing (a worktree picker) waits for evidence anyone wants it.
