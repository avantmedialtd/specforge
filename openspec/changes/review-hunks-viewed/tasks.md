## 1. Core: hunk bodies

- [x] 1.1 `crates/openspec-core/src/diff.rs`: make the hunk reader report each hunk's **body** range in the text it read. The body runs from just after the `@@` header line to the end of the hunk's last line, each line's marker and newline and a `\ No newline at end of file` line included. One reader builds both the hunk and its range (design D2):
  - Add a sibling of `parse_hunks` that returns each `Hunk` with its body range, and build `parse_hunks` on it.
  - `SpannedFile` gains its hunks' body ranges within the input, in order, across both sections of a folded type change.
  - Add a sibling of `diff_versions` that also reports each hunk's body bytes from the patch text it generates.

  `Hunk` and every IPC type stay unchanged. (`pull-request-viewer`: *Review Progress*)
- [x] 1.2 Test the ranges in `crates/openspec-core/tests/` (the targets `diff.rs` already uses):
  - one hunk and two hunks;
  - the header and section heading left out;
  - a no-newline line on either side included;
  - a CRLF line kept with its `\r`;
  - a Latin-1 byte kept raw;
  - a folded type change's two sections;
  - a body slice that re-parses to the same lines;
  - `diff_versions`' bodies equal to slices of its patch text.

  `parse_hunks`' existing tests stay green unchanged.

## 2. App: digests in the cache

- [x] 2.1 `crates/openspec-app/src/pull_request_detail.rs`: `ReadFile` and `CachedFile` gain `hunks: Option<Vec<[u8; 32]>>`, the SHA-256 of each hunk's body, in hunk order. It is `None` for a file without patch text. (design D2)
  - `github_detail.rs` digests the bodies of each `patch` where it takes `PatchDigest`.
  - `bitbucket_detail.rs` digests them from the diff bytes within each `SpannedFile`'s hunk ranges.
  - Budget-withheld files keep theirs, like shown ones.
- [x] 2.2 `crates/openspec-app/src/pull_request_cache.rs`: `keep_fetched` also stores the digests of a fetched file's hunks, under the same generation and commit checks. A fetched too-large or binary file stores none.
- [x] 2.3 Tests:
  - A GitHub file's digests equal the SHA-256 of its body slices.
  - A BitBucket file holding `0xE9` and one holding `0xE8` differ in their digests, though their lines decode the same.
  - A withheld file keeps its digests.
  - A patch-less file has none until a file read is kept, and none from a read kept under another generation.

## 3. App: hunk marks in review progress

- [x] 3.1 `crates/openspec-app/src/review_progress.rs`, the keys:
  - Add `hunk_keys(cached) -> Option<Vec<String>>`, each `sha256:<hex>#<n>` with $$n$$ numbering identical bodies in order. It is `None` when the digests are absent or empty: hunks not known.
  - `Entry` gains `hunks: BTreeMap<String, Vec<String>>` with `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`.

  (`pull-request-viewer`: *Review Progress*; design D1, D3)
- [x] 3.2 `review_progress.rs`, the writes: replace the store's `mark`/`unmark` with one write that applies D3's four rows to a path's file key and hunk keys, under the same lock, re-read and atomic rename.
  - A file mark stores every current hunk key when they are known, and leaves the stored ones otherwise.
  - A hunk mark stores the file key once it covers every hunk.
  - A hunk unmark first writes out the hunks viewed through the file key.
  - Every write to a file with known hunks prunes its keys to its current hunks.
  - `lastMarkedHead` advances on any mark.
  - An unmark never creates an entry.
- [x] 3.3 `review_progress.rs`, the states (design D4, D6):
  - `progress` derives each file's state by D4's cases, adding `FileReviewState::PartlyViewed`.
  - `FileReviewProgress` gains `hunks: Option<Vec<bool>>`.
  - `ReviewProgress` gains `head_commit` and `base_commit` from the cached detail.
  - The counts come from the states, and a partly viewed file counts in neither.

  Mirror all three in `src/types.ts` (`FileReviewState`, `FileReviewProgress`, `ReviewProgress`) in the same step.
- [x] 3.4 Table-driven tests in `review_progress.rs`, one fixture per D3 row and per D4 case. Add the adversarial fixtures a whole-function mutant won't separate:
  - twin hunks marked apart;
  - a rebase that moves every hunk without changing its body, keeping the file viewed;
  - a file mark from before hunk marks (key, no hunks) then a hunk unmark, keeping the other hunks;
  - a stale file key surviving hunk marks until the last;
  - a stale hunk key pruned by a later mark;
  - an unknown-hunks file with stored hunk keys, partly viewed;
  - a stale file key with every current hunk viewed, viewed;
  - an entry with no `hunks` key reading unchanged, and a written entry whose `hunks` is empty leaving the key out.
- [x] 3.5 `crates/openspec-app/src/service.rs`:
  - Add `set_hunk_viewed(reference, path, hunk, viewed, head, base)`, with `set_file_viewed`'s refusals plus unknown hunks and an index past the last. It computes the key from the cached detail and raises `review-progress-changed` on a stored write.
  - `set_file_viewed` keeps its signature and writes D3's file rows.
  - `review_progress` answers the new fields.
  - Tests:
    - each refusal stores nothing;
    - a stored hunk mark notifies;
    - a hunk mark on a patch-less file works once its file read is kept;
    - a hunk unmark with no entry creates none;
    - the answer names the detail's commits.

  (design D5)
- [x] 3.6 `crates/openspec-app/tests/wire_shape.rs`: pin `partlyViewed`, `hunks` as an array or `null`, and `headCommit`/`baseCommit` in `review_progress()` and its key test, against the `src/types.ts` mirror from 3.3.

## 4. Shells

- [x] 4.1 Register `set_hunk_viewed` in three Rust places:
  - `crates/specforge/src/commands.rs`: a `#[tauri::command]` that only deserialises and calls the service.
  - `crates/specforge/src/lib.rs`: the `generate_handler!` list.
  - `crates/specforge-web/src/dispatch.rs`: a match arm.

  Add a `dispatch::tests` case: refused, storing nothing, without a cached detail, as `set_file_viewed`'s is. (`pull-request-viewer`: *Review Progress*, *Transports*)

## 5. Frontend: the diff view

- [ ] 5.1 `src/components/DiffView.tsx`: add the `hunkSlots?: (file) => HunkSlots | undefined` prop.
  - The heading extra goes in each hunk heading row's gutter, left of the `@@` text: inside the unified heading's padding, and inside the side-by-side heading cell.
  - An end row renders after the hunk's last line only when the slot returns content: a row in `.diff-unified-lines`, and a full-width `tr` side by side.
  - All of it carries `data-copy="skip"`.

  (`diff-view`: *Diff View Hosts*; design D7)
- [ ] 5.2 `DiffView.tsx`, folds:
  - A folded hunk renders one row: the heading extra, the `@@` heading, and a "Show n lines" button with `aria-expanded` and the name "Show n lines from new line N" (old line for an all-removed hunk). No line rows and no end row.
  - Shown overrides are view state per file key and hunk index. A new `files` array drops them, as it drops loads. A hunk's override drops when the host's fold of it changes.
  - Folding never re-tokenises: `hunkTokens` stays once per hunk.

  (`diff-view`: *Folded Hunks*)
- [ ] 5.3 `DiffView.tsx`: accept a `ref` (React 19 prop) exposing `DiffViewHandle.collapse(file)`, which adds the file's key to the existing collapse state, so the header toggle reverses it. (`diff-view`: *File Sections*)
- [ ] 5.4 `DiffView.tsx`, place keeping: extend `recordPlace`/`restorePlace` with D8's anchors.
  - The activated heading row.
  - The first row after a hunk, for its end row.
  - The section header, for a collapse.
  - Otherwise the topmost line, falling back to its folded heading or collapsed header.

  Record and restore in a layout effect around each fold, show, hide and collapse. A layout switch keeps folds and overrides, and keeps a folded heading at the top when it was the topmost row. (`diff-view`: *Folded Hunks*, *Side-Qualified Line Identity and Switching*)
- [ ] 5.5 `src/diffLayout.ts`: `modelCopyText` takes which hunks are folded and not shown, and skips their lines. Extend `diffLayout.test.ts` with a selection from hunk 1 to hunk 3 across a folded hunk 2, in unified and on a named side. (`diff-view`: *Selection and Copying*)
- [ ] 5.6 `src/App.css`: the folded row, the gutter checkbox, the end row and the "Show"/"Hide" control, each visible at rest without hover in both themes. The gutter checkbox must fit inside the unified heading's existing gutter padding, so the `@@` text stays aligned with the code.

## 6. Frontend: the pull-request view

- [ ] 6.1 `src/api.ts`: `setHunkViewed(reference, path, hunk, viewed, head, base)`.
- [ ] 6.2 `src/pullRequestView.ts`, each with tests in `pullRequestView.test.ts`:
  - `viewedMark` gains the mixed box state and D11's header words.
  - `hunkStates(progress, detail, path)` returns `null` unless the progress names the detail's head and base commits.
  - `endRowFor(lines, viewed)` is true when unviewed and over 40 lines.
  - The hunk checkbox's accessible name.

  (`pull-request-viewer`: *Changed Files in the Pull-Request View*)
- [ ] 6.3 `src/components/PullRequestView.tsx`, marks and slots:
  - `ViewedToggle` becomes tri-state, through `indeterminate`, so it is exposed as mixed. A mixed or unchecked box marks.
  - A hunk-marks hook mirrors `useViewedMarks`: pending state shown until the progress lands, and a refused mark's reason in its heading row until it is re-marked or a new detail arrives.
  - The `hunkSlots` supply each hunk's checkbox, the end row's "Mark hunk viewed", and folds for every viewed or pending-viewed hunk.
- [ ] 6.4 `PullRequestView.tsx`, collapse and reload:
  - Hold the `DiffView` ref. After a mark made in this view, collapse a file once the progress that follows shows it viewed. Remote notices and opening collapse nothing.
  - Re-read progress after a load brings a file's hunks.

  (design D9)

## 7. Notes

- [ ] 7.1 `crates/CLAUDE.md`: the hunk reader's body ranges, `CachedFile.hunks`, and `review_progress.rs`'s hunk keys and write rule. `src/CLAUDE.md`: `set_hunk_viewed` among the commands, and `DiffView`'s hunk slots, folds and collapse handle beside its per-file slots.

## 8. Verification

- [ ] 8.1 `bun run build`, then `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test`, all green. `bun test` green.
- [ ] 8.2 Mutation-test the changed `openspec-core` and `openspec-app` files as CI does, never with `--baseline=skip`. Check `outcomes.json` durations against the 90-second limit under `RUST_TEST_THREADS=2`. If the macOS `document_watch` flake fails the baseline, split the run per crate. Record the counts here.
- [ ] 8.3 Smoke the browser skin (`specforge-serve` from a debug build with an isolated `APP_IDENTIFIER`, plus `bun run dev`) against a real GitHub pull request with a large file, and walk these scenarios, recording each:
  1. Mark a hunk: it folds, the box goes mixed, and the header reads "1 of n hunks viewed".
  2. Mark a long hunk from its end row: the next hunk stays put.
  3. "Show" at the top keeps the heading in place.
  4. Mark the last hunk: the section collapses.
  5. A mixed box marks the file.
  6. Copy across a folded hunk.
  7. A layout switch keeps folds, and a folded heading on top.
  8. A second tab's mark folds the hunk without collapsing or moving anything.
  9. A restart keeps the marks.
  10. Loading a patch-less file brings its hunk checkboxes.
  11. In the isolated `review-progress.json`, swap one stored hunk key for a stale one and mark the file's key stale: the header reads "changed since viewed · 1 hunk to review" with only that hunk unfolded.

  Delete the isolated state afterwards.
