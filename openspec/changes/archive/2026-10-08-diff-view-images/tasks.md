## 1. Core: what an image is, and a commit's blobs

- [x] 1.1 Add `imagesize = { version = "0.15", default-features = false, features = ["png", "jpeg", "gif", "webp", "bmp", "ico", "heif"] }` to `crates/openspec-core/Cargo.toml`. It is already in `Cargo.lock` through `resvg`. Confirm the lockfile resolves it to the same 0.15.x.
- [x] 1.2 New `crates/openspec-core/src/image.rs`, pure, re-exported from `lib.rs`. (`diff-view`: *Image Comparison*)
  - `IMAGE_PIXELS_LIMIT = 40_000_000`. The byte ceiling is the existing `REQUESTED_FILE_BYTES_LIMIT` (8 MiB).
  - `is_lfs_pointer(bytes)`: under 1,024 bytes, first line `version https://git-lfs.github.com/spec/v1`, an `oid sha256:` line of 64 hex digits, a `size` line of digits, and only `ext-` lines besides.
  - `inspect(bytes) -> ImageCheck`, either `Image { mime, width, height }` or `Refused(reason)`, deciding `Lfs`, `TooLarge`, `NotImage`, `TooManyPixels` in that order. The seven signatures are those of design D3. AVIF is `imagesize`'s `Heif(Av1)`, and any other HEIF is `NotImage`. Dimensions come from `imagesize::blob_size`; an ICO's are its largest entry's.
- [x] 1.3 Test `image.rs`:
  - each signature, from a tiny real fixture;
  - HEIC refused while AVIF is accepted;
  - an ICO with 16, 32 and 256 entries reporting 256 × 256;
  - the LFS grammar: valid, a short `oid`, 1,024 bytes, no `version` line;
  - exact boundaries: 8 MiB accepted and one byte more refused; 40,000,000 pixels accepted and one more refused, by a crafted PNG `IHDR`;
  - the refusal order: a 9 MiB non-image is `TooLarge`.
- [x] 1.4 `crates/openspec-core/src/git.rs`: add `commit_file_blobs(common_dir, sha, path, old_path) -> Result<FileBlobs, CommitReadError>`, as in design D4. (`commit-graph`: *Commit Detail View*, *Commit References Are Injection-Safe Arguments*)
  - Get the parents from `commit_base`.
  - Run `diff-tree -r -z --raw -M --end-of-options <first parent> <sha>`, or `--root` for a root commit, with `-- :(literal)<path> [:(literal)<old_path>]`. Parse the old and new object ids; an all-zero id means the side is absent.
  - Run `cat-file --batch-check` with the ids on stdin.
  - Run `cat-file --batch` for only the ids within the ceiling.
  - Each side is `Absent`, `TooLarge { size }` or `Bytes(Vec<u8>)`. No path, and no value from the caller other than the validated `sha`, reaches `cat-file`.
- [x] 1.5 Test `commit_file_blobs` over a fixture repository:
  - a modified image, and an image added in a root commit and in a later commit;
  - a deleted image, and a renamed and changed image;
  - a merge read against its first parent;
  - `pages/[id].png` read beside `pages/i.png`;
  - a blob past 8 MiB answering `TooLarge` with its size and no bytes;
  - a malformed `sha` refused before `git` runs.

## 2. App: wire shapes and the commit read

- [x] 2.1 `crates/openspec-app/src/pull_request_detail.rs`: add the types of design D10, every tagged enum with `rename_all` and `rename_all_fields`. (`diff-view`: *Image Comparison*; `pull-request-viewer`: *Pull-Request Image Reads*)
  - `ImageVersions`, `ImageSide` (`image` with base64 `data` from the `base64` crate, `absent`, `refused`) and `ImageRefusal`.
  - `PullRequestImageOutcome` (`images`, `changed`, `failed`).
  - `FileReadFailure::Redirected`.
  - Mirror every one in `src/types.ts`, and add `"redirected"` to its `FileReadFailure` union.
- [x] 2.2 `crates/openspec-app/tests/wire_shape.rs`: a fixture per variant of each new type. Assert `tooLarge`, `notImage`, `tooManyPixels`, `lfs` and `redirected` by exact wire value.
- [x] 2.3 `crates/openspec-app/src/service.rs`: add `commit_file_image(repo_id, sha, path, old_path) -> Result<ImageVersions, String>`. It authorises the repository at the shared boundary as `commit_file` does, runs `commit_file_blobs` on the blocking pool, and maps each side through `image::inspect`. Test that an unregistered repository is refused with no `git` run, and that a malformed `sha` is refused. (`commit-graph`: *Commit Reading Is Restricted to Registered Repositories*)

## 3. App: the pull-request image read

- [x] 3.1 `crates/openspec-app/src/pull_request_cache.rs` (`pull-request-viewer`: *Pull-Request Image Reads*):
  - `image_fetch(key, path, head, base) -> Result<ImageFetch, FileChanged>` gives the file's old and new paths, its status, the commits and the cached merge base. It answers `FileChanged` exactly when `file()` does.
  - `keep_merge_base(key, fetch, merge_base, generation, enabled) -> bool` keeps the merge base under `keep_fetched`'s checks.
  - Test each refusal path as `keep_fetched`'s tests do.
- [x] 3.2 `crates/openspec-app/src/github_detail.rs`: add `read_images_with(pull_request, fetch, get, get_raw, clear, limits, now)`. It reuses `compare_url`, `contents_url` and their verdicts. It sends the compare only without a merge base, the old contents unless the file was added, and the new unless it was deleted. It returns each side's raw bytes and the merge base.
- [x] 3.3 `crates/openspec-app/src/bitbucket_detail.rs` (`pull-request-viewer`: *Pull-Request Image Reads*; `bitbucket-pull-requests`: *Privacy and Safety*):
  - `merge_base_url` and `src_url`, built from the row's workspace and repository, with each path segment percent-encoded, following the artifex recipe's encoding (`reference_artifex_bitbucket_reference_impl`).
  - The merge-base verdict: JSON whose `hash` is 40 hex digits, else transient.
  - The raw verdict: bytes to 8 MiB plus one.
  - Statuses: a 3xx is `ReadEnd::Redirected` (new), 401 and 403 are unauthenticated, 429 sets the shared deadline, and 404 is unavailable.
  - `read_images_with`.
- [x] 3.4 `crates/openspec-app/src/pull_request_read.rs`:
  - A raw BitBucket read: `bitbucket_detail::Body::Raw` on `DetailIo::bitbucket_get`, read to 8 MiB plus one byte. `LiveIo`'s goes through `usage_http::get_without_redirects` with basic auth, and the fake gets scripted `merge-base` and `src` replies.
  - `read_pull_request_image(context, reference, fetch, io) -> PullRequestImageOutcome`: resolve the credential, be admitted by the provider's limits, send through `clear_to_send`, run the recipe, `keep_merge_base`, then `image::inspect` each side.
  - `file_failure` maps `Redirected`.
- [x] 3.5 `crates/openspec-app/src/service.rs`: add an async `pull_request_file_image(reference, path, head, base)`. A disabled provider answers `failed` with `refused`, and the read runs on the blocking pool.
- [x] 3.6 Tests over the fake transport, for each provider:
  - **GitHub:** three requests, then two once the merge base is known, including after a file read of the same detail. An added file sends one contents GET.
  - **BitBucket:** the `merge-base` GET, then both `src` GETs. A redirect answers `redirected` and requests nothing more.
  - **Budget and deadlines:** a spent budget defers with no request. A 429 sets the GitHub REST deadline, or BitBucket's shared deadline.
  - **Other failures:** a 404 is `unavailable`, and a push read since is `changed` with no request.
  - **Credentials and caching:** a credential saved mid-read keeps nothing. A second read sends its version requests again, but no compare and no `merge-base`.
  - **Sniffing:** HTML bytes make a `notImage` side.

## 4. Shells

- [x] 4.1 `crates/specforge/src/commands.rs`: add `get_commit_file_image` and an async `get_pull_request_file_image`, both thin calls into `AppService`. Register both in `crates/specforge/src/lib.rs`'s `generate_handler!`.
- [x] 4.2 `crates/specforge-web/src/dispatch.rs`: an arm for each command. Extend `dispatch::tests` so both are known on the web transport. (`pull-request-viewer`: *Pull-Request Image Reads*, *Transports*)

## 5. Frontend: decisions

- [x] 5.1 `src/api.ts`: add `getCommitFileImage(repoId, sha, path, oldPath)` and `getPullRequestFileImage(reference, path, head, base)`.
- [x] 5.2 `src/diffFiles.ts`, bun-tested (`diff-view`: *Image Comparison*):
  - `IMAGE_EXTENSIONS`, and `isImageFile(file)` implementing design D1's predicate: case-insensitive, on the new path or the deleted file's old path.
  - `isLfsPointerFile(file)`, from the hunks' lines.
  - Update `contentStateLabel`'s doc.
  - Tests:
    - each extension;
    - SVG excluded, and a `.png` with text hunks excluded;
    - a GitHub 0/0 `.png` included;
    - LFS pointer hunks on one side and on both.
- [x] 5.3 New pure `src/diffImage.ts`, bun-tested:
  - `decodeBase64`.
  - `formatBytes` ("812 B", "345 KB", "1.4 MB").
  - `sizeDelta` ("−47 KB (−14%)").
  - `differenceAvailable`.
  - `refusalText` for each reason, plus "Can't be shown here".
  - The caption and the accessible names ("icons/app.png at abc1234").
  - The near-view queue: at most two in flight, in arrival order, dropping a section that left the margin.

## 6. Frontend: the view

- [x] 6.1 `src/components/DiffView.tsx` (`diff-view`: *Image Comparison*, *Diff View Hosts*, *Unified and Side-by-Side Layouts*):
  - **Props.** Add the optional `readImage?: (file) => Promise<ImageVersions>` and `imageReads?: "nearView" | "onRequest"`.
  - **Read state.** Per file key, dropped with a new model.
  - **Before a read.** The "Stored in Git LFS" row, "Show image" or "Try again" with the reason, and the placeholder. In `nearView`, an `IntersectionObserver` rooted at the scroll port with a one-viewport margin feeds the queue from `diffImage.ts`.
  - **`ImageComparison`.**
    - Halves when side by side, stacked in unified, one full-width version for an added or deleted file.
    - Captions, the checkerboard, and `width`/`height` attributes.
    - Object URLs created per mount and revoked on unmount; `onError` shows "Can't be shown here".
    - The Both/Difference radio group (one Tab stop, wrapping arrows), offered only for equal dimensions.
    - Per-file mode state, dropped with a new model.
  - **The reader's place.** `recordPlace`/`restoreAnchor` around the swap from placeholder to comparison.
  - **Copying.** Its controls are left out of copies.
- [x] 6.2 Styles beside the diff view's: checkerboard tokens for light and dark (both theme blocks), the two halves, the stacked versions, and the Difference frame (black, `isolation: isolate`, `mix-blend-mode: difference`).
- [x] 6.3 `src/components/CommitDetailView.tsx`: pass `readImage` → `getCommitFileImage` with the commit's id, and `imageReads="nearView"`. (`commit-graph`: *Commit Detail View*)
- [x] 6.4 `src/components/PullRequestView.tsx` and `src/pullRequestView.ts` (`pull-request-viewer`: *Changed Files in the Pull-Request View*):
  - **The reader.** `readImage` maps the outcome: `changed` → `onFileRefused`, and `failed` rejects with `fileFailureText`, which gains `redirected`. Pass `imageReads="onRequest"`.
  - **The host link.** Keep per-file image state so the preamble's `hostFileLink` covers an image file that is an LFS pointer, failed as `redirected` or `unavailable`, or has a refused or undrawable side.
  - **Tests.** The wording and the link decision.

## 7. Notes

- [x] 7.1 `crates/CLAUDE.md`:
  - `image.rs`, and `commit_file_blobs` beside the commit reads.
  - The image read among the app's runtime network calls in the `usage_http.rs` paragraph.
  - `image_fetch` and `keep_merge_base` in `pull_request_cache.rs`'s entry.
- [x] 7.2 `src/CLAUDE.md`:
  - The diff view's image reader and its two timings.
  - `diffImage.ts` among the pure modules.
  - `get_pull_request_file_image` among the pull-request view's shared commands.

## 8. Verification

- [x] 8.1 `bun install && bun run build`, then `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test` and `bun test`, all green. Because `git.rs` changed, also run `cargo check --target x86_64-pc-windows-msvc -p openspec-core --all-targets`.
- [x] 8.2 Mutation-test the changed lines as CI does (`cargo mutants --in-diff` against `origin/master`). Check `outcomes.json` durations against the 90-second limit with `RUST_TEST_THREADS=2`. For each survivor, add the assertion or a reasoned exclusion in `.cargo/mutants.toml`, never `--baseline=skip`.
- [x] 8.3 Smoke the browser skin against isolated state: a debug `specforge-serve` with a scratch identifier, driven from a visible Chrome window. A hidden automation tab stalls `IntersectionObserver`; use the known shim if one must be used. Walk these:
  - **An icon-regeneration commit of this repository** (`git log --diff-filter=M -- 'crates/specforge/icons/*.png'`):
    - opening it reads no image;
    - images read as their sections near the view, two at a time, side by side and stacked;
    - Difference on an equal-size icon;
    - collapse and expand read nothing;
    - an image loading above the reader keeps their place.
  - **A commit adding an image:** one version only.
  - **A GitHub pull request with a changed PNG:**
    - "Show image" makes three requests, and a second image makes two;
    - a deferred read's wording;
    - the pull-request window renders images under its content-security policy.

  Record what was not walked: BitBucket (no token on this machine; covered over the fake transport), Git LFS (no registered LFS repository; covered by tests), and formats a platform cannot draw.

  Walked 2026-10-07: a debug `specforge-serve` built with a scratch `APP_IDENTIFIER` (reverted), on port 4391, in a hidden Chrome tab with a stand-in `IntersectionObserver` that re-checks on DOM mutations.
  - **`c7701f2` (Regenerate brand icons, 50 image files):**
    - opening it sent one `get_commit_detail` and no image read;
    - with a 748 px port, only 4 files read, every unread one more than a viewport below;
    - at most 2 reads in flight;
    - unified stacked and side by side in halves, old left, every `<img>` a `blob:` URL;
    - Difference on `128x128.png`: black, isolated, `mix-blend-mode: difference`, named by path and both sides, kept across a layout switch;
    - collapse then expand read nothing and made fresh object URLs;
    - six images loading above `proposal.md` (+1,603 px) left its line 10 within 1 px of where it was.
  - **`971c605`:** `docs/screenshot.png` read only once scrolled near (it started 5,058 px down). One full-width version, no Difference, named "docs/screenshot.png at 971c605", scaled to its column.
  - **`avantmedialtd/avantmedia#19` (16 baseline PNGs GitHub sends without a patch, 0/0):**
    - every one offered "Show image", none read "No textual changes", and none read on open;
    - each click made one read, rendered in halves captioned by branch with sizes and changes;
    - no progress read after an image read;
    - the pull-request window (`?pullRequest=1`) decoded both versions under its CSP with no violation.
  - **Not observable live:** GitHub's REST counter (`gh api rate_limit`) did not move for the server's REST GETs, so the 3-then-2 request count rests on `a_github_image_read_sends_three_requests_and_then_two`. A deferred read was not provoked live; its wording is pinned by `fileFailureText`'s test.
- [x] 8.4 Desktop smoke in `bun run wt:dev`: the center pane and a pull-request window each show a pull request's changed image, and commit detail shows an icon commit.

  Walked 2026-10-07 in the WKWebView (slot 1, scratch `APP_IDENTIFIER`), through a temporary `index.html` probe; both temporary edits were reverted.
  - **Commit detail, `c7701f2`:** 17 comparisons read near the view, with 33 files still waiting further down. All 34 `<img>` were `blob:` URLs and drawn, none "Can't be shown here".
  - **Center pane, `#19`:** "Show image" on the iPhone 15 Pro baseline drew both versions.
  - **Pull-request window:** the iPad Air baseline drew both versions under its CSP, with no violation.

## 9. Zoom: the maximized comparison (design D11)

- [x] 9.1 `src/components/figureZoom.ts`: an optional scale ceiling on `clampScale`, `zoomAt` and `actualSizeState`, defaulting to `MAX_SCALE`, so the figure view is unchanged. Test the ceiling at both values.
- [x] 9.2 New pure `src/imageZoom.ts`, bun-tested (`diff-view`: *Image Comparison*, Zooming):
  - `IMAGE_MAX_SCALE = 32`;
  - the extents holding both versions;
  - the opening state: fit, held within the bounds;
  - whether a scale draws pixels sharp (above 1).
- [x] 9.3 New `src/components/ImageZoom.tsx`: a modal `<dialog>` with the figure view's toolbar (zoom out, scale, zoom in, fit, 1:1, close) plus the Both/Difference group.
  - One frame, or two in lockstep: one scale, every offset written to both.
  - Wheel and pinch zoom at the pointer, drag pans, Escape or a click outside the versions closes.
  - `ObjectImage` moves to its own module for both components.
- [x] 9.4 `src/components/DiffView.tsx`: a zoom control on each comparison, left out of copies, and a click on a version, both opening the dialog. The dialog's mode is the file's mode, and it closes with the comparison.
- [x] 9.5 Styles beside the image styles: the two frames, their captions, and sharp pixels above actual size.
- [x] 9.6 Notes: the maximized comparison in `src/CLAUDE.md`.
- [x] 9.7 Verify:
  - `bun run build` and `bun test`;
  - `cargo` is unaffected;
  - in the running app: open an icon and a screenshot maximized, zoom and pan in lockstep, sharp pixels, the 32× stop, Difference there, Escape closes with the diff unmoved, and no read.

  Walked 2026-10-07: `bun test` (1,080) and `bun run build` passed, with no Rust change. On the scratch `specforge-serve`, against `c7701f2`'s `128x128.png`:
  - opening by the Zoom control and by a click on a version both worked, at 614% with two frames captioned `4c5774c` and `c7701f2`, `pixelated`;
  - a wheel zoom kept both frames at one scale and one pair of offsets, and continued zooming stopped at 3,200%;
  - scrolling the right frame moved the left;
  - Difference in the dialog showed one blended frame at the same scale and switched the section too;
  - Escape closed with no read and the diff unmoved.

  `971c605`'s screenshot opened in one frame with no Difference choice, and 1:1 went to 100% with smoothing back on. The walk found `zoomAt` mis-anchoring a figure smaller than its frame, which is centred there. It now accounts for that inset, tested, and the figure view gains the fix too. The `wt:dev` window took every change by hot reload.

## 10. Zoom: a window of its own, under the platform's gestures (design D11, revised)

- [x] 10.1 New pure `src/imageWindow.ts`, bun-tested:
  - the window's source, a commit's or a pull request's, and its address, a canonical string;
  - parsing that address back, refusing anything malformed;
  - the browser tab's path and name, and the window's title.
- [x] 10.2 `src/windowKind.ts` and `src/main.tsx`: the `imageWindow` kind, with the pull-request window's content-security policy and a surface of its own. Test the kind and its policy.
- [x] 10.3 Desktop shell:
  - `crates/specforge/src/image_window.rs`: the third `DetachedKind`, `image-<hash>`, at a fixed size clamped to the work area, its title sanitised and following the page.
  - `open_image_window` in `commands.rs` and `lib.rs`, and `capabilities/image.json` granting `core:window:allow-close` alone.
  - Extend the capability tests.
  - Pin `open_image_window` as unknown on the web transport in `dispatch::tests`.
- [x] 10.4 `src/api.ts`: `openImageWindow(address, title)`, the desktop command, or a named tab in the browser skin.
- [x] 10.5 `src/components/ImageZoom.tsx` becomes the window's surface, with no `<dialog>`:
  - two-finger scroll and the wheel pan natively, and frames follow one another;
  - a pinch zooms at the pointer, as WebKit's `gesture*` events or as a wheel with `ctrlKey`;
  - ⌘ or Ctrl with `+`/`=`, `-`, `0` and `9`;
  - the first frame takes focus, so the keyboard scrolls it.
- [x] 10.6 New `src/components/ImageWindowRoot.tsx`. It reads the source through `getCommitFileImage` or `getPullRequestFileImage`, says why when it cannot, and builds the frames. It closes on Escape and Cmd/Ctrl-W through `useDetachedWindow`.
- [x] 10.7 `DiffView`: a `zoomImage` host prop behind the Zoom control and the click on a version. `CommitDetailView` and `PullRequestView` open their source's window.
- [x] 10.8 Styles: the window's layout. Notes: `src/CLAUDE.md` and `crates/CLAUDE.md` on the third window kind and the fifth desktop-only command.
- [x] 10.9 Verify:
  - `tsc`, `bun test`, `bun run build`;
  - for the shell crates, `cargo fmt`, clippy and their tests;
  - in the running app: a commit's and a pull request's zoom window, a second Zoom focusing it, gestures by synthetic events, and Escape.

  Walked 2026-10-08:
  - **Gates:** `tsc` clean, `bun test` 1,091, `bun run build`. `cargo test -p specforge -p specforge-web`, their clippy `-D warnings` and `cargo fmt --check` all pass.
  - **Browser skin, opening:** commit detail's Zoom control and a click on a version both open `/?imageWindow=1&at=…`, under one tab name, with no in-page dialog.
  - **Browser skin, the window** (`c7701f2`'s `128x128.png`): `surface=image`, the content-security policy, and two frames captioned with sizes, at fit (614%) with the first frame focused.
  - **Gestures, by synthetic events:**
    - a plain wheel was not intercepted and left the scale unchanged;
    - a wheel with Control zoomed to 916%;
    - WebKit `gesturechange` to 2 zoomed to exactly twice that, its default prevented;
    - both frames held one set of offsets, and scrolling one moved the other;
    - ⌘=, ⌘−, ⌘0 and ⌘9 went to 2,290%, 1,832%, 100% and fit;
    - Escape, ⌘W and the Close control each closed the window.
  - **A pull request's window** (`#19`'s iPhone 15 Pro baseline): read through `get_pull_request_file_image`, captioned `master` and `insights-rewrite/code-factories`, at fit (14%).
  - **The `wt:dev` window:** it rebuilt with `open_image_window` and relaunched.
  - **Not walked:** a real trackpad pinch and two-finger scroll in the desktop WKWebView, and a native `image-*` window opening. Those are left to the user's own try in the running app.

## 11. Review fixes (2026-10-08, after the first push)

- [x] 11.1 A path that would leave its commit is not read. A pull request's file paths come from the provider's payload, and a `.` or `..` segment survives the per-segment encoding, so the API host would resolve it to another of its paths, credential and all. `FetchPaths::addressable` refuses such a path, and BitBucket's `read_images_with` and GitHub's shared `read_versions_with` end `Unavailable` before sending anything. That covers GitHub's file reads too, whose `contents_url` predates this change.
- [x] 11.2 Every canvas a decoder may size a version by is counted. `imagesize` reads one header per format, while a web view's decoder may size its canvas by another, so a crafted file could declare 3 × 2 to the check and 30,000 × 30,000 to the decoder. `inspect` now also counts these:
  - a GIF's screen grown to hold each frame;
  - each icon image by its own PNG or bitmap header;
  - each JPEG frame header found as libjpeg finds markers, past fill bytes, standalone markers and stray bytes.

  A PNG, or a PNG inside an icon, whose first chunk is not `IHDR` (Apple's `CgBI`) is `notImage`. Each walk is bounded by its byte count, so no mutant can hang it. Each test asserts that `imagesize` alone reads the small size, then that the version is refused.
