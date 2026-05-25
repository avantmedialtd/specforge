# Design

## Context

SpecForge today treats each registered folder as an independent OpenSpec workspace. The registry is a path-keyed map, the cache is `HashMap<PathBuf, Vec<ChangeData>>`, and each workspace gets its own `notify` watcher on `<path>/openspec/changes/`. There is no concept of git, branches, repos, or worktrees anywhere in `openspec-core` or the shell — it's pure filesystem.

The user's working environment has shifted under that assumption. Two new realities:

1. **Manual worktree-per-change workflow**: the user's `into-worktree` skill creates a git worktree per OpenSpec change so several in-flight changes can be edited and tested in parallel without branch-swapping.
2. **Harness-driven worktrees**: the Claude Code background-job harness creates ephemeral worktrees under `.claude/worktrees/` for every parallel job. These come and go on the order of minutes-to-hours, often touching OpenSpec changes as part of the work.

The result is that SpecForge — whose entire premise is "ambient awareness of what's in flight" — is blind to most of what's in flight. The user explicitly asked for this change to surface what Claude is doing across worktrees, not just to support a manual workflow nicety. That framing shapes several decisions below: auto-discovery has no skips, the harness's worktree paths are first-class, and the active-indicator semantics privilege "what's hot right now."

## Goals / Non-Goals

**Goals:**

- Detect when a registered workspace is inside a git repo and surface every worktree of that repo as a sibling instance under one logical change tree.
- Pick up new worktrees the moment they appear on disk (harness creates them constantly) and drop them when they disappear, with no user action.
- Show divergence between a non-default-branch instance and the default-branch instance with a visible label, so the user can tell at a glance whether work has diverged or is stale relative to the merged trunk.
- Keep the aggregation in `openspec-core` so the tray badge, notifications, and frontend all see the same join — never duplicate the logic at consumers.
- Preserve existing behaviour for non-git workspaces (the parser doesn't know git is involved and shouldn't have to).

**Non-Goals:**

- A UI affordance for creating or removing worktrees. The user has CLI tooling (`into-worktree` skill, raw `git worktree`) and this change doesn't try to compete with that.
- Diff viewer for divergent change content. The label is enough; viewing the diff is a future change.
- Cross-repo change identity. A change called `add-auth` in repo A and repo B remains two unrelated logical changes.
- Multi-user / remote awareness. We don't fetch, we don't compare against `origin/<branch>` content, we don't surface "PR open" state.
- Per-instance activity attribution. "Claude vs human" is not a distinction the model makes; the most-recently-modified instance gets the active dot regardless of who modified it.
- Notifications per instance event. With harness churn this would be a constant stream of noise.

## Decisions

### Repo identity = canonical path to git common dir

What identifies "the same repo across worktrees" is the common git directory — the one shared by every worktree's `.git` pointer file. `git rev-parse --git-common-dir` returns it; canonicalising the result gives a stable `RepoId` that's identical for every worktree of one repo and never collides across unrelated repos (even ones with the same folder name).

Workspaces that aren't in a git repo keep `RepoId = None` and stay flat — the existing top-level `WorkspaceFolder` node, no aggregation, no instance children, no divergence labels. This preserves behaviour for OpenSpec workspaces that aren't version-controlled (rare but possible).

**Alternatives considered:** Identifying by the main worktree path. Rejected because the main worktree path is also a worktree — there's no special "main" git directory; every worktree has an equally valid `.git` pointer to the same common dir. Using `--git-common-dir` is the canonical answer git itself uses.

### Aggregator lives in `openspec-core`, above the cache

The cache stays dumb — `HashMap<PathBuf, Vec<ChangeData>>`, one entry per registered or discovered workspace. A new module (`repo_view.rs`) reads the cache and produces a `Vec<WorkspaceView>` where each variant is either:

```rust
enum WorkspaceView {
    Repo(RepoView),                             // git-backed, aggregated
    Flat(WorkspaceFolder, Vec<ChangeData>),     // not in git, as today
}

struct RepoView {
    id: RepoId,
    main_worktree: PathBuf,
    name: String,
    default_branch: Option<String>,
    logical_changes: Vec<LogicalChange>,
}

struct LogicalChange {
    name: String,                       // change directory name = ID
    instances: Vec<ChangeInstance>,     // sorted: most-recently-modified first
}

struct ChangeInstance {
    worktree_path: PathBuf,
    branch: Option<String>,             // None if not on a branch (detached, bare)
    is_main_worktree: bool,
    is_default_branch: bool,
    is_archived_here: bool,
    change: ChangeData,                 // existing parser output, untouched
    modified_at: SystemTime,
    divergence: Option<DivergenceLabel>,
}

enum DivergenceLabel {
    Diverged,           // content differs from default-branch instance
    StaleVsArchived,    // active here, archived on default branch
}
```

The aggregator is a pure function from `(Vec<(WorktreeInfo, Vec<ChangeData>)>, Option<DefaultBranch>) → RepoView`. Trivially testable without the watcher or the shell.

**Why not in the cache itself?** Would entangle two concerns: tracking on-disk state per path (cache) and joining views across paths (aggregator). The cache shape change would also ripple through every consumer.

**Why not in the frontend?** The tray badge needs the count of logical changes; notifications need to fire on logical-level events. If only the frontend joins, the shell would have to re-implement the join — and the two implementations would inevitably drift.

### Worktree auto-discovery — no skips, by design

When a workspace is registered (and is inside a git repo), `git worktree list --porcelain` enumerates every worktree of that repo. Every worktree that's not already in the registry is added with origin `Discovered { discovered_via: <RepoId> }`. Harness worktrees under `.claude/worktrees/` are included — the user explicitly chose this because the whole point of the feature is visibility into what Claude is doing.

User-registered vs discovered are persistence-distinct:
- **User-registered** entries persist to the registry config file. The user removes them via Settings.
- **Discovered** entries are recomputed at startup from the user-registered set's repos and from the meta-watcher. They never persist.

Edge: if the user registers a worktree directly (not the main worktree), the registry-driven repo scan finds the *main* worktree among the discovered siblings. Both end up tracked, with the user-registered one as user-registered and the rest as discovered. Removing the user-registered one cascades a removal of every discovered worktree of the same repo.

**Alternatives considered:**
- Skip `.claude/worktrees/` by default. Rejected — that's exactly the visibility the user wants. A future setting could expose a filter list if the noise becomes a problem.
- Per-worktree opt-in. Rejected — defeats the "ambient" goal; the user would forget to opt in to harness worktrees and miss what they care about most.

### Default branch detection cascade

```
1. git symbolic-ref --short refs/remotes/origin/HEAD    → "origin/main" → "main"
2. git config --get init.defaultBranch
3. branch checked out in main worktree at first detection
4. None (no instance is tagged is_default_branch)
```

Cached per repo. Refreshed when a watcher fires on `.git/refs/remotes/origin/HEAD` or `.git/config`. Both are tiny files and rarely change, so the cost is negligible.

Why a cascade rather than picking one? Each step has failure modes that the next fixes: no remote (step 1 fails), no `init.defaultBranch` (step 2 fails — most users don't set it), brand-new repo with no remote and no init config (step 3 picks whatever was checked out). Step 4 is the bottom: don't fake a default; no instance gets the marker.

**Implication for divergence labelling**: when no default branch is known, no instance can be the reference, so no divergence labels are computed. Acceptable — better than mislabelling.

### Dynamic worktree tracking via meta-watcher

A `notify` watcher is installed on each repo's `.git/worktrees/` directory (non-recursive — we only care about top-level adds and removes). When it fires, the discovered-worktree reconciler runs:

1. Re-enumerate via `git worktree list --porcelain`.
2. Compute the diff against the current discovered set.
3. For each new worktree: register it as discovered, install its `openspec/changes/` watcher, parse it, emit appropriate events.
4. For each removed worktree: tear down its watcher, drop it from the registry, emit appropriate events.

Filtering: the meta-watcher fires on git's own lock files and metadata too. The reconciler is idempotent — running it on a spurious event is a no-op if nothing actually changed. Cheap enough to not worry about over-firing.

**Edge: worktree directory deleted but `.git/worktrees/<name>/` lingers.** This is the `git worktree remove` vs `rm -rf` distinction. `git worktree list --porcelain` lists worktrees whose path is missing as `prunable`. The reconciler treats prunable worktrees as removed, even if `git worktree prune` hasn't run yet.

### Singleton flattening (option 2)

The default tree shape (Sketch 2c) puts a disclosure-only parent row above each logical change with its instances as children. For changes that exist in exactly one worktree — the common case for a fresh proposal that hasn't been branched off — the parent row is pure visual overhead. The render rule:

- 1 instance: render as a single instance row, no parent, no disclosure.
- ≥2 instances: render the parent disclosure row with instance children underneath.

The tree shape mutates when an instance is added or removed. This is intentional — when Claude opens a worktree and creates an instance of a change that previously existed only on master, the user sees the row visibly *promote* from "one place" to "two places." That mutation is the signal.

**Alternative considered: always show the parent.** Rejected — most changes are single-instance most of the time, and forcing an expand-to-see-anything every click would feel like ceremony.

**Alternative considered: auto-expand singletons.** Rejected — same visual cost (vertical space, indent) without the click savings.

### Per-instance divergence label

Computed per non-default instance against the default-branch instance:

```
no label    → change doesn't exist on default (branch-only — the common in-flight case)
no label    → change exists on default, content identical
[diverged]  → change exists on default, content differs
[stale]     → archived on default, still active here
```

"Content differs" is a byte-comparison of every file under `openspec/changes/<name>/**`. Cached per `(logical_change, instance)`. Invalidated when a file event fires on either side of the comparison.

Edge: a logical change with no default-branch instance and multiple non-default instances. No reference point exists, so no labels. Acceptable — divergence-against-default is the only divergence semantics we ship.

### Active indicator: most-recently-modified

Each logical change picks one primary instance: the one with the most recent `modified_at` (= max mtime over `openspec/changes/<name>/**`). The primary gets a ● dot in the tree.

Why "most recent": matches the user's mental model of "what's being worked on now." Other candidates (most task progress, default branch, longest-lived branch) all have valid arguments but mtime is the only one that updates in real time as work happens. With the framing of "show what Claude is doing," the dot is effectively a heartbeat.

### Logical change is "archived" only when every instance is

A change can be in `openspec/changes/archive/` on one worktree and still active on another (the typical "PR merged, branch not yet caught up" state). The aggregator treats:

- Any instance non-archived → logical change is active, lives in the Active section.
- All instances archived → logical change is archived, lives in the Archive section.

Active logical changes with some archived instances show those instances with the `[stale]` label. This is the intended way to surface "your branch is behind master's archive — go merge or delete."

### Tray badge: count logical changes

The badge has always been "non-archived count." With logical changes, the only sensible meaning is: count of distinct `(repo_id, change_name)` tuples that have at least one non-archived instance. A change touched by 3 worktrees still contributes 1.

Counting instances instead would inflate the badge whenever the harness was busy, making the indicator useless. The semantic the user cares about is "things in flight," not "files on disk."

### Notifications: logical-level only

`ChangeAdded` notifies today on first appearance of a change directory. With instance churn from the harness, an instance-level `ChangeAdded` would fire every time a harness worktree opened and touched a change. So:

- `LogicalChangeAdded` (a change ID first appears anywhere across the repo's worktrees) → notify.
- `LogicalChangeArchived` (every instance is now archived) → notify.
- `InstanceAdded` / `InstanceRemoved` / `Updated` → silent.

This is a deliberate narrowing. If the user later wants "tell me when a new worktree starts working on something," that's a future feature with an opt-in setting.

### Shell out to `git`, no `git2`/`gix` dependency

The git operations we need are all short and read-only: `rev-parse --git-common-dir`, `symbolic-ref refs/remotes/origin/HEAD`, `worktree list --porcelain`, `branch --show-current`, `config --get init.defaultBranch`. None of them benefit from a libgit binding.

Adding `git2` would pull in `libgit2` (C dependency, build-system pain on Windows). Adding `gix` would add ~hundreds of KB of compile time and binary size for features we don't use.

Shelling out via `std::process::Command` is fine:

- The output formats are stable and well-documented (`--porcelain` exists precisely for this).
- `git` is present on every developer machine that would run SpecForge.
- If `git` is missing or returns an error, repo detection fails for that workspace and it falls back to flat treatment — the app keeps working.

A thin `git` module in `openspec-core` wraps the calls so the rest of the code never spawns processes directly.

## Risks / Trade-offs

- **[Worktree path canonicalisation across symlinks]** — macOS routinely resolves `/Users/...` via firmlinks; the registry already canonicalises paths and the aggregator must use the same canonical form to match instances back to repo entries. Audit at every comparison site.
- **[Spurious meta-watcher events from git's lock files]** — `notify` will fire on `.git/worktrees/<name>/HEAD.lock` and friends. The reconciler must be idempotent under arbitrary fire frequencies. Cheap; not a correctness risk, only a wasted-CPU risk.
- **[Divergence comparison cost on large change directories]** — byte-comparing every file across multiple instances on every event could get expensive for a change with many large spec files. Mitigate by caching per `(logical_change, instance)` and only recomputing when a file event touches the relevant tree. Re-evaluate if profiling shows pain.
- **[`git` not on PATH]** — for users without git installed, every git workspace degrades silently to flat. We should log this at startup (once per missing repo) and consider a settings-page indicator. Not fatal.
- **[Harness worktree noise]** — if many parallel jobs touch the same change, the tree could show many instance rows. The most-recently-modified sort keeps the active one on top, but expansion of the parent could be unwieldy. Acceptable for v1; revisit if it bites.
- **[Persistence migration]** — old config files don't have an `origin` field. Deserialise with `#[serde(default)]` so old entries default to `UserRegistered`. Forwards compatibility (newer files read by older builds) is automatic since the extra field is ignored.
- **[Submodules]** — `git rev-parse --git-common-dir` correctly returns the *outer* repo's git directory for a workspace inside a submodule. The submodule's own worktrees won't appear as siblings of the parent repo's worktrees. Correct behaviour, but worth verifying with a test fixture.
- **[Bare and non-default-branch main worktree]** — main worktree's branch may not be the default branch (someone working on a feature branch in the main checkout). The cascade still finds the default via `origin/HEAD`; the main worktree's instance is simply not tagged `is_default_branch`. This is correct but unusual to look at — worth checking the UI doesn't degrade.

## Open Questions

- **Repo display name.** Currently the main worktree's basename. If two different repos happen to share a basename (`backend/` in two projects), the user can't tell them apart at a glance. Probably fine for v1; if it bites, qualify with parent dir.
- **Instance display name.** Branch name (`worktree-tray-glyph-spec-variant`) vs path basename (`tgsv`). Branch name is more informative when worktrees follow the harness convention. Fall back to path basename when branch is unknown.
- **Settings affordance for the discovered set.** Should Settings show discovered worktrees at all (read-only list of "found these via this repo")? Probably yes for transparency, but the v1 settings UI can stay focused on user-registered entries only.
- **Reading from a removed-worktree instance.** If the user has the detail pane open on an instance and the worktree gets pruned mid-render, the next `read_artifact` call will fail. Graceful empty state plus a "this instance is gone" message, then auto-select the primary instance of the same logical change. Catch in implementation.
