# Tasks

## 1. Comparison Decisions and Pixels

- [x] 1.1 Extend `src/diffImage.ts` mode eligibility, labels, slider arithmetic and summary helpers; verify equal/unequal dimensions, missing/refused sides, slider endpoints and wording in `src/diffImage.test.ts` (`diff-view`: *Image Comparison*, *Interactive Image Comparison*).
- [x] 1.2 Add `pixelmatch` in `package.json`/`bun.lock`, bounded RGBA comparison in `src/imageDiffPixels.ts` and `src/imageDiff.worker.ts`; verify identical/changed pixels, transparency, resized edges, counts and the union ceiling in `src/imageDiffPixels.test.ts` (`diff-view`: *Bounded Highlighted Image Diffs*).

## 2. Shared Image Surfaces

- [x] 2.1 Add `src/components/ImageSurface.tsx` and local decoding/worker lifecycle helpers; verify slider pointer/keyboard isolation, blink pause/reduced-motion/visibility cleanup, decode failure and stale/cancelled work with focused tests (`diff-view`: *Interactive Image Comparison*, *Bounded Highlighted Image Diffs*).
- [x] 2.2 Integrate six modes and aligned version frames into `src/components/DiffView.tsx` and `src/styles/base.css`; verify mode switching without new reads, dimension changes, added/deleted/refused sides and per-model reset in `src/components/DiffView.test.tsx` (`diff-view`: *Image Comparison*).

## 3. Zoom and Guidance

- [x] 3.1 Reuse the modes and surface in `src/components/ImageWindowRoot.tsx` and `src/components/ImageZoom.tsx`; test parity, retained scale/scroll offsets and returning lockstep frames (`diff-view`: *Image Comparison*).
- [x] 3.2 Update `src/CLAUDE.md` for the new shared viewer and pixel-processing boundary; verify guidance matches implemented mode/worker ownership.

## 4. Verification

- [x] 4.1 Run `bun run lint`, `bun test`, `bun run build` and `openspec validate playwright-style-image-review --strict`; resolve regressions and record outcomes.
- [x] 4.2 After the frontend build, run `cargo test --workspace`; record any environment blocker rather than changing unrelated code.
- [x] 4.3 Run the app with `bun run wt:dev` and inspect all six modes, resized/transparent images, slider touch/keys, blink pause/reduced motion and zoom/pan; also check served-browser behavior with the real backend where available. Record evidence and any unverified platform paths.

## 5. Review Fixes

- [x] 5.1 Correct EXIF-oriented JPEG dimensions in `src/diffImage.ts` before deriving shared frames or decoding comparison pixels; add orientation and pixel-decoding regressions in `src/diffImage.test.ts` and `src/imageDiff.test.ts` (`diff-view`: *Image Comparison*, *Bounded Highlighted Image Diffs*).
- [x] 5.2 Retain the intended zoom position through mode-induced scroll clamping in `src/components/ImageZoom.tsx`; add clamped-offset and subsequent-pan regressions in `src/components/ImageWindowRoot.test.tsx` (`diff-view`: *Image Comparison*).

## 6. Review-Fix Verification

- [x] 6.1 Run focused and full frontend tests, lint, production/native builds and OpenSpec validation; browser-check a real EXIF-rotated JPEG and right-edge mode switching, and update verification notes and design guidance.

## 7. Spec Validation

- [x] 7.1 Extract the mode contract into *Image Comparison Modes* in `specs/diff-view/spec.md`, preserve all existing scenario identities and safety rules, and verify strict validation is warning-free.

## Verification Notes

### Initial Implementation (2026-11-22)

- `bun run lint`: passed.
- `bun test`: **1,617 passed**, zero failures across 85 files.
- `bun run build`: passed, including strict TypeScript and the bundled image-diff worker. Existing large-chunk warnings remain.
- `git diff --check`: passed.
- Normal OpenSpec validation passes. `openspec validate playwright-style-image-review --strict` reports only the inherited long-text warning on the existing *Image Comparison* requirement. Baseline `openspec validate diff-view --type spec --strict --json` reports the same warning on that requirement and 17 other existing requirements. The modified block is intentionally retained whole with its scenario identities, rather than silently trimming the existing contract or broadening this change into a spec split.
- Rust workspace: **2,102 passed**, zero failures, including wire-shape and web/server suites. Command: `env -u GH_TOKEN -u GITHUB_TOKEN -u BITBUCKET_USERNAME -u BITBUCKET_API_TOKEN cargo test --workspace --quiet`. An initial run with inherited credential variables hit two unchanged environment-sensitive tests; no Rust code was changed. One existing assertion prints its credential on failure; the user was alerted to rotate that credential, and no credential value is copied into this change.
- `bun tauri build --debug --no-bundle`: passed again after final frontend changes.
- Ran `bun run wt:dev` in slot 1 (`localhost:1430`): native compilation and startup completed, but its embedded server reported the existing `4317` listener. Visual testing used a worktree Vite server against that existing real backend, with synthetic checked-version image replies only for the test fixtures. The shared `DiffView` and real `ImageWindowRoot` were mounted, not a second renderer. No PR review state or shared application settings were changed.
- Browser smoke, desktop and 390px viewport: all six modes present; differently sized 720×420 / 800×460 versions share scale and alignment; transparent areas retain their checkerboard. Highlighted Diff generated the expected red button recolor and resized borders, with a pixel-count summary.
- Slider: mouse reveal 30%, touch reveal 70%, Home 0% and End 100%; no unintended zoom window or panning. Modes reused the one image read; opening the independent zoom root made its one additional governed read.
- Zoom: at 244%, scrolling moved both frames together; switching through Slider and back retained both offsets at `(780, 358)` and the scale. Plain wheel panned without zooming. At 390px the toolbar wrapped, every control remained available and the page had no horizontal overflow.
- Blink: alternation and Pause/Resume inspected; emulated reduced motion started paused. A 16×16 icon retained its natural pixels with readable, separate controls. Regression tests cover every theme's line-height token, hidden/document visibility, timer cleanup and stale/cancelled worker results.
- Processing limit: a 2200×2000 image pair showed the four-million-pixel limit message with no output canvas, while Slider and the other modes remained usable without another read.
- **Unverified platform path:** native WebKit visual interaction could not be inspected because computer-use screenshot/control tools were unavailable. Native build and browser smoke passed; no claim is made of native visual verification or testing on an actual Safari 14 installation. Screenshots are retained in the session scratch directory, outside git.

### Review Fixes (2026-11-24)

- Added regression tests first: orientation, orientation-aware union bounds, pixel decoding and irreversible scroll clamping reproduced four failures before the fixes. Focused image/component suite now passes **91 tests**.
- Full frontend: **1,624 passed**, zero failures across 85 files; `bun run lint`, `bun run build`, `bun tauri build --debug --no-bundle` and `git diff --check` pass. No Rust implementation changed.
- Normal OpenSpec validation passes; strict validation still reports only the previously recorded long *Image Comparison* requirement warning.
- Launched `bun run wt:dev` in slot 1. Native startup and smoke build passed; native WebKit visual interaction was not reverified. Its embedded browser backend was disabled, so browser checks were repeated with `cargo run -p specforge-web --bin specforge-serve` plus slot-1 Vite. Both curl and the browser confirmed a real `/api/invoke` appearance read returned HTTP 200. Fixtures replaced only accepted-image replies for synthetic fixture references; no real PR/review state or application settings were modified.
- Real JPEG generated with raw 200×100 pixels and EXIF orientation 6: Before, After, Side by side, Slider and Blink rendered natural/display dimensions of **100×200** inline and in zoom. Its oriented PNG equivalent produced **zero changed pixels** on a **100×200** canvas in both Diff surfaces. Captions also report oriented dimensions; switching modes caused no additional read. Screenshot retained as `image-review-fixes-oriented-jpeg.png` in session scratch storage.
- Real browser layout at **244%**: right-edge offsets `(1381.5, 556)` temporarily clamped to `(798, 556)` in each full-width mode, then restored to `(1381.5, 556)` in both returning Side by side frames. A subsequent reader pan in Slider to `(598, 456)` replaced the retained position and both returning frames followed it.

### Spec Validation Cleanup (2026-11-24)

- Extracted the new mode contract into *Image Comparison Modes*: 485 characters and five focused scenarios. The legacy *Image Comparison* text is now 8,934 characters, down from 9,609 and below its 9,138-character main-spec baseline; no existing read, refusal, safety, rendering or zoom contract was removed.
- Compared parsed deltas before and after: every existing scenario name and body is unchanged, and all other existing requirements are unchanged. Only the mode contract moved into the referenced requirement.
- `openspec validate playwright-style-image-review --strict --json`: **passed with zero issues**, resolving the warning recorded above. `git diff --check` also passes. This cleanup changes spec/design/task text only; application code is unchanged.

### Shipping Verification (2026-11-24)

- Repeated `bun run lint`, all **1,624 frontend tests**, and `bun run build`: passed. Rust formatting, workspace clippy with `-D warnings`, and all **2,102 Rust tests** also passed; the Rust test process again excluded inherited credential variables.
- Synced all five delta requirements into `openspec/specs/diff-view/spec.md`: three added and two modified. Parsed requirement/scenario content matches the delta exactly; the existing title, purpose and every unrelated requirement remain unchanged.
- `openspec validate --specs`: **46 passed**, zero failures; existing repository-wide advisory warnings remain. The selected change's strict validation has zero issues.
- No source changes under `openspec-core` or `openspec-app`; mutation testing is out of scope for this frontend-only implementation, as configured by the changed-lines gate.
- Archived-change task lint reports this change valid with zero issues (13/13 complete). The repository-wide archived check also reports 49 older archives with unfinished tasks; those historical files are unchanged and were not marked complete by this work.
