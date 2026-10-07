## 1. Core: the local diff

- [ ] 1.1 Add `similar` to the workspace and to `crates/openspec-core/Cargo.toml`.
- [ ] 1.2 In `crates/openspec-core/src/diff.rs`, add `diff_versions(old: Option<&[u8]>, new: Option<&[u8]>) -> DiffContent`. Rules: past `REQUESTED_FILE_BYTES_LIMIT` per version → `TooLarge`; a NUL byte or invalid UTF-8 → `Binary`; else a line diff with three lines of context and a 2-second deadline, rendered as unified hunks and parsed by `parse_hunks`; diff text past the limit → `TooLarge`. Re-export it from `lib.rs`. (`pull-request-viewer`: *GitHub Detail Reads*)
- [ ] 1.3 Test it: modified, added (old `None`), deleted (new `None`), identical, the no-newline flag on either side, NUL, invalid UTF-8, each ceiling, and a fixture whose counts match `git diff --numstat`.

## 2. App: the file read

- [ ] 2.1 `github_detail.rs`: a patch-less entry with lines becomes `DiffContent::Withheld`, and its `ReadFile` records the old and new paths to fetch. Add `compare_url`, `contents_url` (each path segment percent-encoded), the compare verdict (`merge_base_commit.sha`, 40 hex) and the contents verdict (raw bytes), classified as a files GET is, with 404 or a redirect unavailable. Add `read_file_with`, which sends compare only without a merge base, then the old and new contents as the status needs, asking `clear` before each. (`pull-request-viewer`: *GitHub Detail Reads*)
- [ ] 2.2 `pull_request_read.rs`: add `github_get_raw` to `DetailIo` (status, rate headers, bytes read to `REQUESTED_FILE_BYTES_LIMIT + 1`), `LiveIo`'s through `usage_http::get_without_redirects` with `Accept: application/vnd.github.raw`. Add the fake's scripted compare and contents replies. Add `read_pull_request_file`, which resolves the token, is admitted by `GithubLimits`, sends through `clear_to_send`, and runs `diff_versions`.
- [ ] 2.3 `pull_request_detail.rs` + `pull_request_cache.rs`:
  - `CachedFile.fetch` and a slot for a loaded too-large or binary content.
  - `CachedDetail.merge_base`.
  - `file()` answers `Changed`, a ready file, or what to fetch.
  - `keep_file()` stores a file read's result and the merge base only under the same generation and commits.
  - `PullRequestFileOutcome` (`file`, `changed`, `failed` with `reason`, plus `untilUnix` for `deferred`), with `rename_all_fields`.

  (`pull-request-viewer`: *Detail Reads Are Scoped to the Snapshot*)
- [ ] 2.4 `service.rs`: `pull_request_file` becomes `async` and returns `PullRequestFileOutcome`. A disabled provider answers `failed` with the reason `refused`. A file read runs on the blocking pool.
- [ ] 2.5 Tests over the fake:
  - three requests then none;
  - a second file sends no compare;
  - an added file sends one contents GET;
  - a 429 sets the REST deadline and answers `deferred`;
  - a spent budget defers with no request;
  - a 404 answers `unavailable`;
  - a credential saved mid-read keeps nothing;
  - a push read since answers `changed`.

  Extend `crates/openspec-app/tests/wire_shape.rs` with the outcome.

## 3. Shells

- [ ] 3.1 `crates/specforge/src/commands.rs`: `get_pull_request_file` returns `PullRequestFileOutcome` from the async service call. `crates/specforge-web/src/dispatch.rs`: its arm awaits the same. Update `dispatch::tests` for the outcome.

## 4. Frontend

- [ ] 4.1 `src/types.ts`: mirror `PullRequestFileOutcome`. `src/api.ts`: `getPullRequestFile` returns it.
- [ ] 4.2 `src/pullRequestView.ts`:
  - `fileFailureText(reason, untilUnix)`, in the view's words;
  - `hostFileLink(provider, page, file)`, with `src/sha256.ts` (FIPS 180-2 vectors);
  - tests for both.

  (`pull-request-viewer`: *Changed Files in the Pull-Request View*)
- [ ] 4.3 `src/components/PullRequestView.tsx`:
  - the loader reads the outcome, and only `changed` calls `onFileRefused`;
  - `failed` rejects with its wording;
  - a too-large file's preamble carries the host link, opened as the view's other provider links are.

## 5. Notes

- [ ] 5.1 `crates/CLAUDE.md`: `diff.rs`'s `diff_versions`, the file read among the network calls, and `pull_request_cache.rs`'s file-read slots. `src/CLAUDE.md`: `get_pull_request_file`'s outcome. Also `src/diffFiles.ts`'s `contentStateLabel` doc ("Only a withheld file's row offers a control").

## 6. Verification

- [ ] 6.1 `bun run build`, then `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test`, all green. `bun test` green.
- [ ] 6.2 Mutation-test the changed files as CI does, checking `outcomes.json` durations against the 90-second limit with `RUST_TEST_THREADS=2`.
- [ ] 6.3 Smoke the browser skin against isolated state, with GitHub on the account `gh` uses, and walk four scenarios:
  - load avantmedialtd/avantmedia #18's `ci-qa-ai-development/+Page.tsx` (158 added and 1,477 removed lines, and a second load sends nothing);
  - a deferred load says when and reads nothing;
  - a too-large file's host link;
  - an unchanged withheld file still loads from the cache.
