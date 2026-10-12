## 1. Read model (openspec-app)

- [x] 1.1 In `crates/openspec-app/src/pull_request_detail.rs`:
  - add `ReadLinks { diff, diffstat }` and `ReusedFiles { files: Vec<DiffFile>, cached: Vec<CachedFile>, unlisted_files: u32, merge_base: Option<String> }`;
  - change `ReadParts.files` to `ReadFiles::{Listed(Vec<ReadFile>), Reused(ReusedFiles)}`;
  - add `ReadParts.links: Option<ReadLinks>`;
  - update every constructor and test helper (`parts_with`, the providers' `parts`).

  None of these cross IPC, so `src/types.ts` is not touched.
- [x] 1.2 Make `assemble` apply `eager_files`/`keep` only to `ReadFiles::Listed`, and pass `ReadFiles::Reused` through as the detail's files, the cached files and `unlisted_files` (`pull-request-viewer`: *Detail Re-reads Reuse Unchanged Files*).
- [x] 1.3 Test `assemble` with a reused list that holds a withheld file, a fetched file and hunk digests. Assert that the detail and the cached files come back byte-for-byte, and that the budgets are not re-applied.

## 2. Cache reuse and merge (openspec-app)

- [x] 2.1 In `crates/openspec-app/src/pull_request_cache.rs`, add `links: Option<ReadLinks>` to `CachedDetail`, and add `reused: bool` and `links` to `ReadDetail` (design D1, D3).
- [x] 2.2 Add `PullRequestDetails::reusable(key, generation, head, base, links) -> Option<ReusedFiles>`. Under the lock it uses `Cache::get` (no LRU touch) and checks:
  - an entry exists;
  - `generation(provider) == generation`;
  - `head_commit` and `base_commit` are equal;
  - `links` are equal.

  It clones the files, cached files, `unlisted_files` and `merge_base` (design D2).
- [x] 2.3 Change `store` to merge against the live entry under the lock (design D3):
  - same head and base: take the live `merge_base`;
  - reused read with the same head and base: also take the live `detail.files` and `files`;
  - a reused read with no matching live entry keeps its snapshot;
  - a listed read with different commits stores `merge_base: None`.

  The generation and enabled check stays first.
- [x] 2.4 Write deterministic tests for `reusable`, one per condition, so every `&&`/`==` mutant on the predicate is killed:
  - matching entry: `Some`;
  - head differs, base differs, links differ (`Some` vs `Some`, and `Some` vs `None`), no entry: `None` each;
  - stale generation with the entry still present: `forget`, store a fresh entry under the new generation, then probe with the old generation and expect `None` (a probe right after `forget` alone hits the "no entry" branch and leaves the generation `==` mutant alive);
  - `reusable` does not reorder the LRU (a 33rd insert still drops the same entry).
- [x] 2.5 Write tests for `store`'s merge:
  - same commits: keeps the live `merge_base`;
  - different head only, and different base only, as two separate cases: each drops it (one case per side, so an `&&` → `||` mutant on the commit match is killed);
  - a `keep_fetched` interleaved between `reusable` and `store` of a reused read is still present after `store`;
  - a reused read stored after its entry was evicted stores its snapshot;
  - a listed read with the same commits replaces the files but keeps the `merge_base`;
  - a reused read under an advanced generation is not stored.

## 3. GitHub reads (openspec-app)

- [x] 3.1 In `crates/openspec-app/src/github_detail.rs` `read_with`:
  - add a `reuse: impl FnOnce(&str, &str) -> Option<ReusedFiles>` parameter, called once with `headRefOid`/`baseRefOid` after `query_verdict` succeeds;
  - on `Some`, send no files GET and return `ReadFiles::Reused` with `unlisted_files` from the snapshot (`pull-request-viewer`: *GitHub Detail Reads*).
- [x] 3.2 Bound the files loop by `/files/totalCount`: pages up to `min(max(ceil(n/50), 1), 20)` when the total is present, else `DETAIL_MAX_FILES_PAGES`. Keep the `entries.len() < FILES_PER_PAGE` break, and keep `FILES_PER_PAGE` at 50 (design D5). Update the module and `read_with` doc comments that describe the page loop, and the `CachedDetail.merge_base` comment ("a new read of the pull request drops it").
- [x] 3.3 Update `a_full_page_asks_for_the_next_and_a_short_one_ends_the_list`: 100 of 100 files now sends 2 GETs, not 3. Add table cases:
  - totals absent, 0, 1, 49, 50, 51, 100, 1,000 and 1,200, asserting the GET count and `unlisted_files`;
  - total 120 with a 30-entry second page sends no third GET.
- [x] 3.4 Add tests that a reuse hit sends exactly one request (the POST) and returns the snapshot's files and unlisted count, and that a reuse miss sends the pages. Assert that `reuse` is called with the query's head and base, and is never called after an unauthenticated or rate-limited query.

## 4. BitBucket reads (openspec-app)

- [x] 4.1 In `crates/openspec-app/src/bitbucket_detail.rs` `read_with`:
  - add a `reuse: impl FnOnce(&str, &str, &ReadLinks) -> Option<ReusedFiles>` parameter, called after both links pass `followable_link` and before the diffstat;
  - on `Some`, skip the diffstat pages and the diff, then still fetch the comments pages and the statuses;
  - fill `ReadParts.links` on every read (`pull-request-viewer`: *BitBucket Detail Reads*; `bitbucket-pull-requests`: *Privacy and Safety*).
- [x] 4.2 Write tests with the fake transport:
  - a reuse hit sends exactly three GETs (PR, comments, statuses) and none to `DIFF`/`DIFFSTAT`;
  - a miss still sends five;
  - a payload with an unfollowable link is `Unavailable` and `reuse` is never called;
  - a 429 on the comments GET of a reusing read sets the shared deadline and sends nothing more;
  - `reuse` receives the payload's commits and links verbatim.

## 5. Wiring (openspec-app)

- [x] 5.1 In `crates/openspec-app/src/pull_request_read.rs`, have `read_github` and `read_bitbucket` take the read's generation and `manual`, and pass `reuse` as `|..| None` when `manual`, otherwise as a closure over `context.details.reusable(&key, generation, ..)` (design D4). In `read_pull_request`, set `ReadDetail.reused` and `links` from the parts.
- [x] 5.2 Write `read_pull_request` tests over the fake `DetailIo`:
  - a second non-manual read with unchanged commits spends exactly one GitHub budget unit (or three BitBucket units) and returns the cached files;
  - a manual read with unchanged commits sends the files pages and keeps the merge base;
  - a changed head sends the pages and drops the merge base;
  - after a file read, a reusing re-read leaves `get_pull_request_file` answering `file` with no request, and `review_progress` still reports that file's hunks as known;
  - after an image read learned the merge base, a reusing re-read leaves the next image read sending only its version GETs (no compare on GitHub, no `merge-base` on BitBucket).

## 6. Verification

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 6.3 `bun install && bun run build` (needed once in a fresh worktree so `dist/` exists), then `cargo test`
- [x] 6.4 `bun run lint`
- [x] 6.5 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Every mutant on a changed line in `openspec-app` must be caught, with no timeouts. Never use `--baseline=skip`.
- [x] 6.6 `openspec validate efficient-pull-request-reads --strict`
- [ ] 6.7 (Not run in the autonomous drain: needs live GitHub/BitBucket credentials. Before release, verify that real BitBucket payload links have the `:{source}%0D{destination}` shape, else BitBucket reuse never fires, and that GitHub's `files.totalCount` matches the REST list length, else GitHub reuse never fires.) Smoke test (no UI change, behaviour only). Run `specforge-serve` with a GitHub credential and open a listed pull request in the browser skin. Post a comment on the provider, wait for the announcement, and confirm the re-read sends only the GraphQL POST (the files list and any loaded patchless file stay as they were). A manual refresh must send the files pages again. If a BitBucket credential is available, repeat the comment check there: it should send three GETs.
