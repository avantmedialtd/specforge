# Authorize Command Path Inputs Against the Registry

## Why

The commands that read git history and OpenSpec artifacts accept the *location*
to read — a repository or a workspace path — straight from the caller and act on
it with **no check that the location is one the user actually registered**:

- `get_commit_graph` / `get_commit_detail` / `get_commit_diff` build a `RepoId`
  from the caller's `repo_id` and run `git --git-dir <repo_id> …`, so a caller
  can read the full commit history, changed-file lists, and diffs of **any `.git`
  directory on the host**.
- `read_artifact` and the archive commands (`list_archived`,
  `archived_artifact_status`) resolve under the caller's `workspace`. The existing
  traversal guard confines reads to *within* that workspace's
  `openspec/changes/…` subtree, but never *to a registered workspace*, so any
  `…/openspec/changes/.../{proposal,design,tasks}.md` or capability spec **anywhere
  on disk** is readable.

Locally this is bounded by the user driving their own app. But the optional
`specforge-web` server exposes this same command surface over `/api/invoke`
(loopback, other browser pages subject to the origin guard, and — with Tailscale
Serve — tailnet peers), turning it into host-wide information disclosure well
beyond the workspaces the user chose to share. It also supplies the valid
`--git-dir` that the git-ref-injection primitive (sibling change
`harden-git-ref-args`) needs to target a specific repository.

A complication the fix has to respect: the two transports are **not** symmetric.
The desktop Tauri commands for commit reading and archive browsing call the core
functions **directly** and don't even take an `AppService`, while the web dispatch
routes the same commands through `AppService`. A guard added only in
`AppService` would secure the web path and leave the desktop path open. The fix
therefore has to converge both transports on one guarded boundary.

## What Changes

- **Define the allowlist from the registry.** A `repo_id` is authorized only if
  it is the git common directory of a registered workspace (the set already
  computed by `WorkspaceRegistry::repos()`). A `workspace` is authorized only if
  its canonical path is a registered/known workspace folder (a
  `WorkspaceRegistry` entry, which already includes discovered sibling worktrees).
- **Enforce at the shared `AppService` boundary.** Add membership checks to
  `AppService::commit_graph` / `commit_detail` / `commit_diff` (repo check) and
  `read_artifact` / `list_archived` / `archived_artifact_status` (workspace
  check), canonicalizing with the same `paths::canonicalize` (dunce) helper the
  registry keys on, and refusing an unregistered path with a clear error instead
  of reading it.
- **Converge the divergent desktop commands onto `AppService`.** Route the Tauri
  `get_commit_graph` / `get_commit_detail` / `get_commit_diff` / `list_archived` /
  `archived_artifact_status` handlers through the guarded `AppService` methods
  (adding the `AppService` state they currently lack), exactly as `read_artifact`
  already delegates. This both closes the desktop hole and removes the code
  duplication that let the two transports drift apart.
- **Fail closed and read-only.** An unregistered or unresolved path yields an
  error (or the existing degrade-to-empty for the graph), never a filesystem or
  git read outside the registered set. No new mutation is introduced.

## Capabilities

### Modified Capabilities

- `commit-graph`: adds that commit-reading operations act only on a **registered
  repository**; a repository identifier that is not the git directory of a
  registered workspace is refused, not read. Complements *Graceful Degradation
  Without Git* (unreadable → empty) with authorization (unregistered → refused).
- `spec-browser`: adds that artifact reads are confined to **registered
  workspaces** — strengthening the existing path-traversal guard, which confines
  reads within a workspace but not to a registered one.
- `archive-browser`: adds that the archive listing and archived-artifact-status
  operations act only on a **registered workspace**, matching the artifact-read
  guarantee.

## Impact

- **Code (`crates/openspec-app/src/service.rs`)**: add a private
  `ensure_registered_repo(repo_id)` and `ensure_registered_workspace(workspace)`
  (canonicalize + membership against `registry.repos()` / registry entries) and
  call them at the top of the six commands; thread the registry (already held) in.
- **Code (`crates/specforge/src/commands.rs`)**: `get_commit_graph`,
  `get_commit_detail`, `get_commit_diff`, `list_archived`,
  `archived_artifact_status` gain `svc: State<'_, AppService>` and delegate to the
  guarded `AppService` methods instead of calling `openspec_core` directly.
- **Code (`crates/specforge-web/src/dispatch.rs`)**: unchanged in shape — it
  already calls `AppService`, so it inherits the guard.
- **Tests**: `openspec-app` unit tests that an unregistered `repo_id` /
  `workspace` is refused while a registered one still succeeds; a canonicalization
  case (verbatim/`..`/symlinked spelling of a registered path resolves to the same
  membership decision).
- **Behavioural note**: `get_changes` reads the in-memory cache keyed by
  workspace and already returns empty for an unregistered path (no fs/git touch);
  it MAY adopt the same guard for uniformity but is not a disclosure vector.
- **Assumption / out of scope**: `register_workspace` remains the intended way to
  widen the allowlist; over the web this makes *who may register* the responsible
  control, which is the web server's origin/login trust boundary (finding #3,
  handled separately). This change assumes that boundary and does not modify it.
  The git-ref option-injection fix is the sibling `harden-git-ref-args` change.
