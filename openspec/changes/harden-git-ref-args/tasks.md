# Tasks — Harden Git Ref Arguments

## 1. Neutralize option injection at the git sink
- [ ] 1.1 In `crates/openspec-core/src/git.rs`, insert `--end-of-options`
  immediately before `sha` in `commit_diff`
  (`["show", "--format=", "--end-of-options", sha, "--", path]`).
- [ ] 1.2 In `diff_tree_lines`, insert `--end-of-options` before `sha`
  (`["diff-tree", "--no-commit-id", "-r", mode, "--end-of-options", sha]`),
  keeping `mode` before the terminator.
- [ ] 1.3 Audit the other ref-taking helpers (`commit_log`, `commit_files`,
  lifecycle/date helpers) for any positional caller-influenced ref and apply the
  same terminator; note any that take only internal/constant refs.

## 2. Validate the ref shape at the boundary
- [ ] 2.1 Add a shared predicate (e.g. `is_object_id(&str) -> bool` matching
  `^[0-9a-fA-F]{4,64}$`) in `openspec-core` (git module) exported for reuse.
- [ ] 2.2 In `crates/openspec-app/src/service.rs`, reject a non-matching `sha` in
  `commit_detail` and `commit_diff` with a clear `Err("invalid commit reference")`
  before calling the core, covering both the Tauri and web transports.

## 3. Tests
- [ ] 3.1 `openspec-core` test: calling `commit_diff` / `commit_files` with
  `sha = "--output=<tmp>"` leaves `<tmp>` non-existent afterward (no file
  written) and returns empty/inert.
- [ ] 3.2 `openspec-core` test: `is_object_id` accepts 7- and 40-char hex,
  rejects `--output=x`, `HEAD`, `:/msg`, empty, and over-length input.
- [ ] 3.3 Regression: a real full and abbreviated sha still return the expected
  diff and file list.

## 4. Verify
- [ ] 4.1 `cargo test -p openspec-core` and `cargo test -p openspec-app` green.
- [ ] 4.2 `cargo build` (workspace) clean; no frontend/type changes needed.
- [ ] 4.3 `openspec validate harden-git-ref-args --strict` passes.
