# Git Worktree Support

## Why

SpecForge is built on the premise that one developer juggles several OpenSpec workspaces in parallel. Increasingly that parallelism happens inside *git worktrees of the same repo* — both because the user adopted that pattern manually (the `into-worktree` skill creates a worktree per change) and because the Claude Code harness spawns ephemeral worktrees under `.claude/worktrees/` for every parallel job.

Today the registry is path-keyed and worktree-blind. A worktree is invisible unless the user explicitly registers its path, and even when registered it appears as an unrelated workspace alongside the main repo — no awareness that they're parallel views of the same project. The user cannot answer the question SpecForge was built to answer at a glance: *which OpenSpec changes are being worked on right now, and where?* When a Claude job opens a worktree, modifies a change, and exits, none of that activity is visible in SpecForge unless the user registered that ephemeral path beforehand.

This change makes worktrees first-class: when you register a git repo, every other worktree of that repo is auto-discovered and tracked, including ones that appear after registration. A change that exists in N worktrees is shown as one *logical change* with N instance children, so the same change isn't duplicated across the tree. Divergence between an in-flight branch and the default branch is labelled visibly. The result is that the tree pane becomes a live view of every parallel piece of OpenSpec work happening on a registered repo — whether driven by the user or by an agent.

## What Changes

- New `git` module in `openspec-core` shelling out to the system `git` binary for: `rev-parse --git-common-dir`, `symbolic-ref refs/remotes/origin/HEAD`, `worktree list --porcelain`, `branch --show-current`, `config --get init.defaultBranch`.
- Workspace registry distinguishes **user-registered** from **discovered** workspaces. User-registered are persisted; discovered are recomputed on startup and as worktrees appear/disappear at runtime.
- On registering a workspace inside a git repo, every other worktree of the same repo is automatically discovered and added. No skips — harness worktrees under `.claude/worktrees/` are included, by design.
- A meta-watcher on each repo's `.git/worktrees/` directory detects worktrees being added or removed, and reconciles the discovered set without user action.
- Default-branch detection cascade: `origin/HEAD` → `init.defaultBranch` → main-worktree's current branch at first detection → none.
- New **aggregator** layer in `openspec-core`, above the cache, that joins per-worktree `Vec<ChangeData>` into per-repo `RepoView` containing `LogicalChange` entries with one `ChangeInstance` per worktree that holds that change.
- New per-instance metadata: branch, is-main-worktree, is-default-branch, is-archived-here, modified-at, and an optional divergence label.
- Divergence labels: `[diverged]` (content differs from the default-branch instance) and `[stale]` (active here, archived on default branch). Computed by byte-comparing the change directories; cached and invalidated on file events.
- Tree shape (Sketch 2c with singleton flattening): repo grouping, logical-change parent rows that are disclosure-only (not selectable), instance leaf rows that are clickable. A logical change with exactly one instance renders as a flattened single row; it promotes to parent + children when a second instance appears.
- Active indicator: the most-recently-modified instance of each logical change carries a ● dot.
- Active-count badge counts *logical changes*, not instances — a change touched by 3 worktrees still contributes 1.
- A logical change is considered archived only when every one of its instances is archived. Until then it stays in the Active section with `[stale]` labels on the archived instances.
- Desktop notifications fire on logical-level events only (a new change ID first appears anywhere, or the last non-archived instance gets archived). Per-worktree instance churn is silent — otherwise harness activity would notify constantly.

## Capabilities

### Modified Capabilities

- `workspace-registry`: gains git-repo detection, default-branch detection, automatic discovery of sibling worktrees, dynamic tracking of worktree add/remove via a meta-watcher, and a `discovered` vs `user-registered` distinction for persistence. Existing manual-registration and persistence requirements carry over for non-git workspaces and for the primary entry of a git repo.
- `spec-browser`: gains repo grouping above workspaces for git-backed entries, logical-change parent rows, instance leaf rows with branch / mtime / divergence chrome, singleton-flattening render rule, and the ● active-indicator on the primary instance. The four-artifact subtree under each instance (Proposal / Specs / Design / Tasks) is unchanged.
- `tray-indicator`: clarifies that the badge counts *logical changes* (one per `(repo_id, change_name)` tuple), and that desktop notifications fire on logical-level adds/archives, not per-instance churn.

### New Capabilities

(none)

## Impact

- **Code (openspec-core)**: new `git` module (~200 lines wrapping `Command`); aggregator module (`repo_view.rs`); registry gains a `WorkspaceOrigin` enum (`UserRegistered` / `Discovered { discovered_via: RepoId }`); watcher manager gains meta-watcher install/teardown per repo; `CacheEvent` gains four new variants (`LogicalChangeAdded`, `LogicalChangeArchived`, `InstanceAdded`, `InstanceRemoved`).
- **Code (specforge shell)**: `commands.rs` gains `get_workspace_views()` returning `Vec<WorkspaceView>`; `read_artifact` already takes a workspace path so no signature change, but the resolved path now comes from an instance not a workspace; `tray.rs` badge updater switches from summing per-workspace `Vec<ChangeData>.len()` to counting non-archived logical changes; `notifications.rs` listens for the logical-level events.
- **Code (frontend)**: `src/types.ts` mirrors the new types; `TreeSelection` grows `repo`, `logicalChange`, `instance` variants; `useWorkspaces` consumes `WorkspaceView` instead of the flat workspace→changes map; `WorkspaceTree` renders the new node shape with the singleton-flatten rule; `DetailPane` resolves the artifact path via the selected instance.
- **Runtime dependencies**: none new in Rust (shells out to `git`); none new in the frontend. `git` must be on `PATH` — if missing, repo detection fails for that workspace and it degrades to the existing flat treatment.
- **Persistence schema**: the registry config file gains an optional `origin` field per entry. Old entries (no `origin`) default to `UserRegistered`. Forwards-compatible — older versions of the app read newer files as flat user-registered workspaces.
- **Watcher budget**: per registered git repo, +1 meta-watcher on `.git/worktrees/` and +1 light watcher on `.git/config` for default-branch refresh. Plus one full instance watcher per discovered worktree (same as today for an explicitly-registered workspace). Comfortably within OS limits even for repos with many worktrees.
- **Out of scope**: creating or removing worktrees from the UI; diffing artifact contents (only labelling that they differ); cross-repo change identity; multi-user / remote-aware behaviour; per-instance activity timelines or who-modified-what attribution.
