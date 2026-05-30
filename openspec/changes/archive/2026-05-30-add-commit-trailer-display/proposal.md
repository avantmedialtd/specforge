# Display git trailers in the commit detail pane

## Why

Every commit this project produces carries git trailers: `/commit-work` stamps `OpenSpec-Id=<change>` on each commit, and Claude co-authorship adds `Co-Authored-By`. The commit-detail view already promises to show "the commit's … full message" (the **Commit Detail View** requirement), but today it renders only the subject — the trailers, the most structured, machine-written part of the message, are dropped before they ever cross the IPC boundary. `commit_log` captures `%s` and nothing of the body. So the one part of a commit message that reliably encodes *why* a commit exists is invisible in the app that browses those commits.

## What Changes

- The Rust core SHALL capture each commit's git trailers — parsed by git itself, not a hand-rolled scan — and carry them through `RawCommit` → `LaidOutCommit` → the `CommitGraph` IPC payload.
- The `commit_log` `git log` invocation SHALL switch to NUL-delimited records so multi-line trailer values can be captured without colliding with the existing newline-delimited record parsing.
- The commit-detail view SHALL render the captured trailers as a labelled list of key/value pairs below the existing parents block, preserving git's order and showing every value when a key repeats.
- Trailers SHALL be displayed as neutral commit metadata: the `OpenSpec-Id` trailer SHALL get no special styling, link, or marker distinguishing it from `Co-Authored-By` or any other trailer — consistent with the rail's standing "no OpenSpec semantics" rule.
- A commit with no trailers SHALL render no trailer section (no empty affordance).
- Scope is capture + display only: no commit↔change linking, no row chips in the rail, no name/email parsing of `Co-Authored-By`.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `commit-graph`: the **Commit Detail View** requirement is tightened — the "full message" it already promises is made concrete for the trailer block, which SHALL be surfaced as neutral key/value metadata.

## Impact

- **Code:**
  - `crates/openspec-core/src/git.rs` — new `Trailer` type; `RawCommit.trailers`; `%(trailers:…)` added to the `commit_log` format; switch that `git log` to `-z` (NUL records).
  - `crates/openspec-core/src/graph.rs` — `LaidOutCommit.trailers`, passed straight through `layout()`.
  - `src/types.ts` — hand-mirrored `Trailer` interface + `trailers` field on `LaidOutCommit` (camelCase, no codegen).
  - `src/components/CommitDetailView.tsx` — a trailers section in the header.
  - `src/App.css` — trailer list styling.
- **APIs / IPC:** the `CommitGraph` payload gains a `trailers` array per commit; additive, no breaking change.
- **Watcher / cache / events:** none. The graph is fetched on demand via `get_commit_graph`; trailers ride the same path. No `WorkspaceCache` or `CacheEvent` involvement.
- **Risk:** the `git log` record-separator switch (`-z`) touches working parsing code; covered by the existing `commit_log` tests plus new trailer tests.
