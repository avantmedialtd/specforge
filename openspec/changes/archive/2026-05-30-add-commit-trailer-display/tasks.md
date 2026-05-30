## 1. Capture trailers in the core

- [x] 1.1 In `crates/openspec-core/src/git.rs`, add a `Trailer { key: String, value: String }` struct with `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` and `#[serde(rename_all = "camelCase")]`.
- [x] 1.2 Add `pub trailers: Vec<Trailer>` to `RawCommit`.
- [x] 1.3 Append `%(trailers:only,unfold,key_value_separator=%x1d,separator=%x1e)` as the final field of `commit_log`'s `--pretty=format`, and add `-z` to the `git log` args.
- [x] 1.4 Change record parsing from `raw.lines()` to `raw.split('\0')`, skipping the trailing empty chunk; parse the new trailers field by splitting on `\x1e`, then each entry once on `\x1d` into a trimmed `{ key, value }`.
- [x] 1.5 Update the format-invariant comment to describe the NUL records + control-byte separator scheme.

## 2. Pass trailers through the layout

- [x] 2.1 Add `pub trailers: Vec<Trailer>` to `LaidOutCommit` in `crates/openspec-core/src/graph.rs`.
- [x] 2.2 Copy `c.trailers` through in `layout()`'s final `LaidOutCommit` construction (alongside `author`/`date`), and add `trailers: Vec::new()` to the test helper `c(...)`.

## 3. Mirror the IPC type

- [x] 3.1 In `src/types.ts`, add `export interface Trailer { key: string; value: string }` and a `trailers: Trailer[]` field on `LaidOutCommit`, with doc comments matching the neighbours.

## 4. Render in the detail view

- [x] 4.1 In `src/components/CommitDetailView.tsx`, render a trailers section in the header below the parents block when `commit.trailers.length > 0`: one row per trailer — `key` then `value` — all uniform, no per-key special-casing.
- [x] 4.2 Add styling for the trailer list to `src/App.css`, consistent with `.commit-detail-meta` / `.commit-detail-parents`; ensure long values wrap or truncate without breaking the header.

## 5. Tests & verification

- [x] 5.1 `cargo test -p openspec-core`: add tests asserting (a) a commit with `OpenSpec-Id` + `Co-Authored-By` yields both trailers in order; (b) a multi-paragraph body where only the last paragraph is trailers yields just the trailers (prose not captured); (c) two `Co-Authored-By` lines yield two entries; (d) a commit with no trailers yields an empty vec.
- [x] 5.2 Confirm the existing `commit_log` tests (history order, decorations, merge parents, empty-outside-repo) still pass after the `-z` switch.
- [x] 5.3 `bun run build` passes (tsc strict, `noUnusedLocals` / `noUnusedParameters`).
- [x] 5.4 Run `bun tauri dev`, select a commit that carries `OpenSpec-Id` (e.g. this change's own commit once made), confirm the detail pane lists its trailers neutrally, and confirm a trailer-less commit shows no section. _Confirmed visually in the worktree build (`specforge-trailers/target/debug/specforge`): the key/value trailer list renders below the Parent line, `OpenSpec-Id` styled identically to `Co-Authored-By`._
