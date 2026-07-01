# Tasks — Authorize Command Path Inputs

## 1. Registry-membership guards in AppService
- [x] 1.1 Add `ensure_registered_repo(&self, repo_id: &Path) -> Result<RepoId, String>`
  in `service.rs`: canonicalize via `openspec_core::canonicalize`, compare against
  the canonicalized `registry.repos()` set, error on miss.
- [x] 1.2 Add `ensure_registered_workspace(&self, workspace: &Path) -> Result<PathBuf, String>`:
  canonicalize, compare against the canonicalized `registry.entries()` folder set,
  error on miss.
- [x] 1.3 Call `ensure_registered_repo` at the top of `commit_graph`,
  `commit_detail`, `commit_diff`.
- [x] 1.4 Call `ensure_registered_workspace` at the top of `read_artifact`,
  `list_archived`, `archived_artifact_status`.

## 2. Converge the divergent desktop commands onto AppService
- [x] 2.1 `commands.rs`: give `get_commit_graph` / `get_commit_detail` /
  `get_commit_diff` an `svc: State<'_, AppService>` param and delegate to
  `svc.commit_graph` / `commit_detail` / `commit_diff`; remove the direct
  `commit_log` / `commit_files` / `commit_diff` core calls.
- [x] 2.2 `commands.rs`: give `list_archived` and `archived_artifact_status` an
  `svc` param and delegate to the guarded `AppService` methods; drop the inline
  core calls (`list_archived_summaries`, `parse_artifact_status`).
- [x] 2.3 Confirm `dispatch.rs` needs no change (already routes through
  `AppService`), so both transports now share the guard.

## 3. Tests (openspec-app)
- [x] 3.1 Unregistered `repo_id` is refused by `commit_graph` / `commit_detail` /
  `commit_diff`; a registered repo still returns results.
- [x] 3.2 Unregistered `workspace` is refused by `read_artifact` / `list_archived`
  / `archived_artifact_status`; a registered workspace still succeeds.
- [x] 3.3 Canonicalization: a registered path spelled with a trailing slash / `..`
  / symlink resolves to the same accept decision (membership isn't spelling-
  sensitive).

## 4. Verify
- [x] 4.1 `cargo test -p openspec-app` and `cargo test -p openspec-core` green.
- [x] 4.2 `cargo build` (workspace) clean; `cargo test -p specforge` (or the shell
  crate) builds with the new `svc` params.
- [x] 4.3 Manual/desktop smoke: selecting a commit and opening the Archive view in
  a registered workspace still works unchanged.
- [x] 4.4 `openspec validate authorize-command-paths --strict` passes.
