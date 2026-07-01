# Design — Authorize Command Path Inputs

## Context

Two families of command accept a *location* from the caller and read it:

- **Commit reading** — `repo_id` (a git common directory `PathBuf`) → `git
  --git-dir <repo_id> …`.
- **Artifact / archive reading** — `workspace` (a directory `PathBuf`) → files
  under `<workspace>/openspec/changes/…`.

Neither checks the location against `WorkspaceRegistry`. The registry already
knows the answer: `repos()` returns the git common dirs of all registered
workspaces, and its entries enumerate every registered/discovered workspace
folder. The fix is to consult it.

## The transport asymmetry (the load-bearing fact)

The same command reaches the core through two paths that do **not** share a
guard point:

```
Command                Web (/api/invoke → dispatch.rs)   Desktop (commands.rs)          Shared guard point?
─────────────────────  ────────────────────────────────  ─────────────────────────────  ───────────────────
read_artifact          svc.read_artifact                 svc.read_artifact              AppService ✓
list_archived          svc.list_archived                 list_archived_summaries (core)  none  ✗
archived_artifact_…    svc.archived_artifact_status      inline parse_artifact_status    none  ✗
get_commit_graph       svc.commit_graph                  commit_log (core), no svc param none  ✗
get_commit_detail      svc.commit_detail                 commit_files (core), no svc     none  ✗
get_commit_diff        svc.commit_diff                   commit_diff (core), no svc      none  ✗
```

Only `read_artifact` already funnels both transports through `AppService`. The
other five let the desktop bypass `AppService` entirely. So a guard placed only
in `AppService` would protect the web transport and leave the desktop wide open —
and duplicating the guard into `commands.rs` is exactly the drift that produced
this gap.

## Decision 1 — One guarded boundary: `AppService`

Put the authorization in `AppService`, where the registry already lives, and make
every transport go through it. `AppService` holds `registry: Arc<Mutex<
WorkspaceRegistry>>`, so the check is local:

```
fn ensure_registered_repo(&self, repo_id: &Path) -> Result<RepoId, String>
fn ensure_registered_workspace(&self, workspace: &Path) -> Result<PathBuf, String>
```

Each canonicalizes the input and checks membership; on miss it returns an error
and nothing is read.

## Decision 2 — Converge the desktop commands onto `AppService`

Route the five divergent Tauri handlers through the guarded `AppService` methods,
mirroring how `read_artifact` already delegates:

- `get_commit_graph` / `get_commit_detail` / `get_commit_diff` gain
  `svc: State<'_, AppService>` and call `svc.commit_graph/…` (which already run
  the git work under `spawn_blocking`), deleting the direct `commit_log` /
  `commit_files` / `commit_diff` calls.
- `list_archived` / `archived_artifact_status` gain `svc` and delegate to
  `svc.list_archived` / `svc.archived_artifact_status`.

This is a net **simplification** (removes duplicated logic) and the correctness
win: one guard now covers both transports, and future commands inherit it by
construction.

## Decision 3 — Membership semantics

- **Repo**: authorized iff `canonicalize(repo_id)` equals some
  `canonicalize(r)` for `r in registry.repos()`. Compare on the canonical form of
  both sides so a differently-spelled but equivalent path (trailing slash,
  `..`, symlink, Windows verbatim `\\?\`) still matches — using the same
  `openspec_core::canonicalize` (dunce) the registry keys on, so we don't
  reintroduce the `std::fs::canonicalize` verbatim-prefix mismatch seen elsewhere
  in the codebase.
- **Workspace**: authorized iff `canonicalize(workspace)` is the canonical path of
  a `registry.entries()` folder. Entries already include user-registered
  workspaces *and* discovered sibling worktrees, which is exactly the set the tree
  is built from, so no legitimate read is refused.

## Decision 4 — Fail closed, stay read-only

- Commit commands: an unregistered repo is refused with an error. (The graph's
  existing *degrade to empty* for an unreadable repo remains for the
  *registered-but-unreadable* case; unregistered is a distinct, refused case.)
- Artifact/archive commands: an unregistered workspace returns an error; no file
  is read. `archived_artifact_status`'s existing `dir_name` sanitization (`/`,
  `\`, `..`) stays.
- No mutation is added anywhere.

## Alternatives considered

- **Guard only in `AppService`, leave desktop as-is** — rejected: misses the five
  desktop commands that bypass `AppService`.
- **Duplicate the guard in `commands.rs` with a `WorkspaceRegistry` State** —
  rejected: two copies of a security check across two crates is the drift that
  caused the bug; convergence is both safer and less code.
- **Guard in the core git/fs functions** — rejected: the core has no registry and
  shouldn't; membership is an application-policy concern, not a git concern.
  (Contrast the sibling `harden-git-ref-args`, whose *argument-safety* guard
  correctly lives in the core because it needs no policy — only the git binary.)

## What this change deliberately does not do

- It does not change the web server's origin/`Host`/Tailscale-login trust boundary
  (finding #3). Over the web, `register_workspace` is still the way the allowlist
  grows, so *who may register* is governed by that separate boundary; this change
  assumes it and layers path-authorization beneath it.
- It does not alter command names, argument shapes, or the TypeScript types — only
  which locations are accepted narrows.

## Testing

- Unregistered `repo_id` → `commit_graph/detail/diff` refused; a registered repo
  still returns its graph/detail/diff.
- Unregistered `workspace` → `read_artifact` / `list_archived` /
  `archived_artifact_status` refused; a registered workspace still succeeds.
- Canonicalization: a registered path presented with a trailing slash / `..`
  segment / symlink / (on Windows) verbatim prefix is accepted (same membership
  decision), proving the guard doesn't split identities.
- Desktop parity: the Tauri handlers, now delegating, return the same results as
  before for registered inputs (no behavioural regression for legitimate use).
