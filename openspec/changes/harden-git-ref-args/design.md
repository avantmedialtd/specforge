# Design — Harden Git Ref Arguments

## Context

`openspec-core::git` shells out to the `git` binary through a `git_command`
chokepoint. Two commit-reading helpers place a caller-influenced revision
positionally:

```
commit_diff(common_dir, sha, path):
    git show --format= <sha> -- <path>

diff_tree_lines(common_dir, sha, mode):        // via commit_files -> commit_detail
    git diff-tree --no-commit-id -r <mode> <sha>
```

The `sha` originates in the frontend and arrives over two transports:

- **Desktop**: `get_commit_detail` / `get_commit_diff` in `specforge/commands.rs`
  call the `git.rs` functions **directly** (they don't even take an `AppService`).
- **Web**: `/api/invoke` → `dispatch.rs` → `AppService::commit_detail` /
  `commit_diff` → the same `git.rs` functions.

So the *only* layer both transports share for these calls is `git.rs` itself.
Any guard that must protect both has to live there (or be duplicated).

## The vulnerability, precisely

`git` treats a leading-dash argument as an option unless an option terminator has
already appeared. `--` terminates *options-vs-pathspecs* but, for `git show`, the
`<sha>` sits **before** the `--`, so `--`-placement does not protect it.
`--end-of-options` is the terminator that forces everything after it to be a
revision/pathspec, never an option.

Concrete exploit (macOS/Linux):

```
sha = "--output=/Users/me/.zshrc"
=> git show --format= --output=/Users/me/.zshrc -- <path>
=> no revision given, defaults to HEAD; --output writes the diff to ~/.zshrc
=> arbitrary file created / truncated / overwritten
```

`diff-tree` similarly interprets a dashed `<sha>` as an option; its `--ext-diff`
family can invoke a configured external program, widening the impact to potential
command execution.

## Decision 1 — Fix at the sink with `--end-of-options`

Insert `--end-of-options` immediately before the ref in both helpers:

```
git show     --format= --end-of-options <sha> -- <path>
git diff-tree --no-commit-id -r <mode> --end-of-options <sha>
```

Rationale:

- It is **transport-agnostic**: it protects the desktop path (which calls
  `git.rs` directly and would be missed by an `AppService`-only guard), the web
  path, the TUI, and any future caller — one edit, total coverage.
- It is the *complete* structural fix: with the terminator in place, no ref value
  can be parsed as an option, closing the class of bug rather than one string.
- `mode` in `diff_tree_lines` is an internal constant (`""`, `-c`, etc.), not
  caller-influenced, and must stay *before* `--end-of-options` because it is
  itself an option; the ref goes *after*. `path` in `commit_diff` is already
  after `--`, so it remains a pathspec and needs no change.

`--end-of-options` is supported by git ≥ 2.24 (2019). SpecForge already depends
on a modern git for porcelain it parses; this is within the supported floor.

## Decision 2 — Validate the ref shape at the boundary

Add a small predicate — a ref is accepted only if it matches `^[0-9a-fA-F]{4,64}$`
(optionally tightened later to a `git rev-parse --verify <sha>^{commit}` resolve)
— applied in `AppService::commit_detail` / `commit_diff` before the core call.

Rationale:

- The frontend only ever sends full 40/64-char hashes read from the graph, so
  hex validation rejects nothing legitimate.
- It gives an early, clear error (`invalid commit reference`) instead of an empty
  diff, and it is defense-in-depth: even if a future caller reorders args or a
  new helper forgets `--end-of-options`, a non-hex ref is already refused.
- It is deliberately **not** the only layer. The sink guard is load-bearing
  because the desktop commands bypass `AppService`; validation there alone would
  leave the desktop path exposed. Both layers ship together.

## Why not other placements

- **Guard only in `AppService`**: misses the desktop commands, which call
  `git.rs` directly. Rejected as insufficient.
- **Guard only in each command/dispatch arm**: duplicative across two crates and
  easy to forget when a new arm is added — exactly how this bug persisted.
- **Shell-escaping**: not applicable — the code already uses argv (`Command`),
  not a shell string; the bug is option parsing, not shell metacharacters. The
  fix is an option terminator, not quoting.

## Testing

- A ref of `--output=<tempfile-that-does-not-exist>` passed to the commit
  functions leaves that path **non-existent** afterward (proves no write), and is
  rejected by the boundary validator.
- A legitimate full sha and an abbreviated (≥4 hex) sha still return their diff /
  file list unchanged (no regression).
- `mode` variants for merges/root are unaffected by the terminator (guards
  against breaking the sibling `authorize-command-paths` / merge-commit work).

## Interaction with `authorize-command-paths`

Independent and composable. This change constrains the *ref*; the sibling change
constrains the *repository* the ref is read from. Neither depends on the other;
either can land first. Together they make the commit surface accept only a
registered repository and a well-formed object id.
