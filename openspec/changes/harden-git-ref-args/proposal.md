# Harden Git Ref Arguments Against Option Injection

## Why

The commit-reading commands accept a commit reference (`sha`) from the frontend
and hand it to `git` **positionally, with no validation and no option
terminator**:

- `commit_diff` runs `git show --format= <sha> -- <path>` (`git.rs`).
- `diff_tree_lines` (behind `commit_files` / `commit_detail`) runs
  `git diff-tree --no-commit-id -r <mode> <sha>` (`git.rs`).

Because `<sha>` is placed before any `--end-of-options` marker, a caller can
supply a value that `git` parses as a **command-line option** instead of a
revision. The highest-impact instance: `git show` accepts `--output=<file>`, and
with no revision left to resolve it defaults to `HEAD`, so a `sha` of
`--output=/Users/<me>/.zshrc` makes `git show` **write/truncate an
attacker-chosen file** with diff content — a general arbitrary file
create/overwrite primitive (clobber a shell rc file or a LaunchAgent → code
execution on next login). `git diff-tree`'s option surface (`--ext-diff`, etc.)
adds further leverage.

This is reachable from every frontend that can send `get_commit_detail` /
`get_commit_diff`: the desktop Tauri command surface, and — when the optional web
server is enabled — the `/api/invoke` endpoint (loopback, or a tailnet peer via
Tailscale Serve). The frontend only ever sends full commit hashes it obtained
from the graph, so nothing legitimate depends on the current permissiveness.

This is the single most serious defect found in the deep review. It is small and
self-contained to fix, and it is being addressed on its own so the fix can land
without waiting on the larger path-authorization work (see the sibling
`authorize-command-paths` change, which restricts *which repository* these
commands may read).

## What Changes

- **Neutralize option injection at the git sink.** In `openspec-core::git`,
  every commit-reading helper that forwards a caller-influenced ref
  (`commit_diff`, `diff_tree_lines`, and any sibling that passes a ref
  positionally) SHALL insert `--end-of-options` immediately before the ref, so
  `git` can never interpret the ref as an option. The file argument in
  `commit_diff` is already after `--` and stays a pathspec.
- **Validate the ref shape at the boundary.** Reject any `sha` that is not a
  plausible git object id (hex, 4–64 chars) before it reaches `git`, returning a
  clear error. This is applied where the value enters the core from a frontend
  (the `AppService` commit methods), giving an early, friendly rejection in
  addition to the sink-level `--end-of-options` guarantee.
- **Two independent layers, on purpose.** `--end-of-options` at the sink is the
  load-bearing guarantee (it protects *all* callers, including the desktop
  commands that call the `git.rs` functions directly and bypass `AppService`).
  Hex validation is defense-in-depth and produces a better error for genuinely
  malformed input.

## Capabilities

### Modified Capabilities

- `commit-graph`: adds an explicit safety requirement that commit references
  supplied to commit-reading operations are treated as data — validated as
  object ids and passed so they can never be interpreted as git options — so no
  reference value can cause git to write, delete, or mutate files or the working
  tree, or invoke external programs. This strengthens the existing *Read-Only
  Operation* guarantee, which today only covers the actions the UI *offers* and
  not the raw argument path.

## Impact

- **Code (`crates/openspec-core/src/git.rs`)**: add `--end-of-options` before the
  ref in `commit_diff` and `diff_tree_lines` (and audit `commit_log` /
  `commit_files` and any other ref-taking helper for the same pattern).
- **Code (`crates/openspec-app/src/service.rs`)**: validate `sha` in
  `commit_detail` / `commit_diff` (a shared `is_object_id`-style check) so both
  the Tauri and web transports reject malformed refs early.
- **Tests (`crates/openspec-core`)**: add cases proving a ref of
  `--output=<tempfile>` does **not** create/modify that file and is rejected or
  inert, and that a legitimate full/abbreviated sha still resolves.
- **No API/type changes**: command names, arguments, and the TypeScript mirror
  are unchanged; only the values accepted narrow. No frontend change required.
- **Out of scope**: restricting *which* repository a commit command may read (the
  caller-supplied `repo_id` allowlist) — handled by `authorize-command-paths`;
  the web server's origin/login trust boundary — handled separately.
