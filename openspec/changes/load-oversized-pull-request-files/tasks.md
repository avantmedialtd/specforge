## 1. Core: the local diff

- [x] 1.1 Add `similar` to the workspace and to `crates/openspec-core/Cargo.toml`.
- [x] 1.2 In `crates/openspec-core/src/diff.rs`, add `diff_versions(old: Option<&[u8]>, new: Option<&[u8]>) -> DiffContent`. Rules: past `REQUESTED_FILE_BYTES_LIMIT` per version → `TooLarge`; a NUL byte or invalid UTF-8 → `Binary`; else a line diff with three lines of context and a 2-second deadline, rendered as unified hunks and parsed by `parse_hunks`; diff text past the limit → `TooLarge`. Re-export it from `lib.rs`. (`pull-request-viewer`: *GitHub Detail Reads*)
- [x] 1.3 Test it: modified, added (old `None`), deleted (new `None`), identical, the no-newline flag on either side, NUL, invalid UTF-8, each ceiling, and a fixture whose counts match `git diff --numstat`.

## 2. App: the file read

- [x] 2.1 `github_detail.rs`: a patch-less entry with lines becomes `DiffContent::Withheld`, and its `ReadFile` records the old and new paths to fetch. Add `compare_url`, `contents_url` (each path segment percent-encoded), the compare verdict (`merge_base_commit.sha`, 40 hex) and the contents verdict (raw bytes), classified as a files GET is, with 404 or a redirect unavailable. Add `read_file_with`, which sends compare only without a merge base, then the old and new contents as the status needs, asking `clear` before each. (`pull-request-viewer`: *GitHub Detail Reads*)
- [x] 2.2 `pull_request_read.rs`: add `github_get_raw` to `DetailIo` (status, rate headers, bytes read to `REQUESTED_FILE_BYTES_LIMIT + 1`), `LiveIo`'s through `usage_http::get_without_redirects` with `Accept: application/vnd.github.raw`. Add the fake's scripted compare and contents replies. Add `read_pull_request_file`, which resolves the token, is admitted by `GithubLimits`, sends through `clear_to_send`, and runs `diff_versions`.
- [x] 2.3 `pull_request_detail.rs` + `pull_request_cache.rs`:
  - `CachedFile.fetch` and a slot for a loaded too-large or binary content.
  - `CachedDetail.merge_base`.
  - `file()` answers `Changed`, a ready file, or what to fetch.
  - `keep_file()` stores a file read's result and the merge base only under the same generation and commits.
  - `PullRequestFileOutcome` (`file`, `changed`, `failed` with `reason`, plus `untilUnix` for `deferred`), with `rename_all_fields`.

  (`pull-request-viewer`: *Detail Reads Are Scoped to the Snapshot*)
- [x] 2.4 `service.rs`: `pull_request_file` becomes `async` and returns `PullRequestFileOutcome`. A disabled provider answers `failed` with the reason `refused`. A file read runs on the blocking pool.
- [x] 2.5 Tests over the fake:
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

- [x] 3.1 `crates/specforge/src/commands.rs`: `get_pull_request_file` returns `PullRequestFileOutcome` from the async service call. `crates/specforge-web/src/dispatch.rs`: its arm awaits the same. Update `dispatch::tests` for the outcome.

## 4. Frontend

- [x] 4.1 `src/types.ts`: mirror `PullRequestFileOutcome`. `src/api.ts`: `getPullRequestFile` returns it.
- [x] 4.2 `src/pullRequestView.ts`:
  - `fileFailureText(reason, untilUnix)`, in the view's words;
  - `hostFileLink(provider, page, file)`, with `src/sha256.ts` (FIPS 180-2 vectors);
  - tests for both.

  (`pull-request-viewer`: *Changed Files in the Pull-Request View*)
- [x] 4.3 `src/components/PullRequestView.tsx`:
  - the loader reads the outcome, and only `changed` calls `onFileRefused`;
  - `failed` rejects with its wording;
  - a too-large file's preamble carries the host link, opened as the view's other provider links are.

## 5. Notes

- [x] 5.1 `crates/CLAUDE.md`: `diff.rs`'s `diff_versions`, the file read among the network calls, and `pull_request_cache.rs`'s file-read slots. `src/CLAUDE.md`: `get_pull_request_file`'s outcome. Also `src/diffFiles.ts`'s `contentStateLabel` doc ("Only a withheld file's row offers a control").

## 6. Verification

- [x] 6.1 `bun run build`, then `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test`, all green. `bun test` green.
- [x] 6.2 Mutation-test the changed files as CI does, checking `outcomes.json` durations against the 90-second limit with `RUST_TEST_THREADS=2`.

  Recorded on 2026-10-07. The first in-diff run tested 89 mutants: 69 caught, 14 unviable, none timed out, and 6 missed. Each miss was fixed:
  - `VERSION_READ_LIMIT`'s two mutants: a test now pins the limit.
  - `send_get_raw` → `None`, the real GET: excluded in `.cargo/mutants.toml` on `send_get`'s terms.
  - Two `||` conditions in `keep_fetched` and one in `diff_versions`: a test now pins each side on its own.
  - The slowest mutant's tests took about 17 s against the 90 s limit.

  Re-running the whole diff then failed on its unmutated baseline. `document_watch`'s `an_unrelated_batch_notifies_nothing`, which this change does not touch, fails four times in six on macOS under the `mutants` profile, and passes eight times in eight under the test profile. The final run was split instead:
  - `openspec-app`'s 72 mutants against its own tests: 60 caught, 12 unviable.
  - `diff.rs`'s 15 against the `diff` target. Its one miss (`>` → `>=` on the diff text's ceiling) is now caught by an exact-ceiling test: 14 caught, 1 unviable.

  CI's Linux run checks the whole diff.
- [x] 6.3 Smoke the browser skin against isolated state, with GitHub on the account `gh` uses, and walk four scenarios:
  - load avantmedialtd/avantmedia #18's `ci-qa-ai-development/+Page.tsx` (158 added and 1,477 removed lines, and a second load sends nothing);
  - a deferred load says when and reads nothing;
  - a too-large file's host link;
  - an unchanged withheld file still loads from the cache.

  Recorded on 2026-10-07. A debug `specforge-serve` ran with a scratch identifier (state deleted afterwards), GitHub on the account `gh` is signed in to, and Chrome.
  - **Walked live:**
    - **#18's page over the API:** it arrived withheld with GitHub's +158 −1,477. Its first load answered 5 hunks of exactly +158 −1,477 in 1.6 s, and the second was served from the cache in 0.03 s.
    - **#14's page in the view:** it read "Diff not loaded +253 −2020 Load diff", where it used to say too large to preview, and "Load diff" rendered its lines.
  - **Found and fixed:** #14's lines first rendered as +339 −2,106, because `similar`'s default Myers marks kept lines of a rewritten page as changed. `diff_versions` now uses `RawMyers`, and after a rebuild the page rendered +253 −2,020, matching its header and GitHub. A synthetic pair pins it (`a_rewritten_page_marks_only_the_lines_a_shortest_edit_changes`).
  - **Covered by tests instead:**
    - **A deferred load:** GitHub was not rate-limited (`a_rate_limited_version_defers_the_load_and_sets_the_rest_deadline`, `fileFailureText`).
    - **A too-large file's host link:** no listed file passes 8 MiB (`hostFileLink` and `sha256.ts`). `DiffView` mounts its sections only after effects, so a server render cannot show the preamble.
    - **A budget-withheld file served from the cache:** `a_withheld_file_is_served_from_the_cache_with_no_request`.
    - **Request counts:** GitHub's `core` counter, read around each load, did not move, so it could not count them. The exact requests are pinned over the fake transport (`a_file_sent_without_its_patch_is_read_once_and_kept`).
