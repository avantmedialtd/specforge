# Pull-Request Review Workspace Tasks

## 1. Presentation State and Working Views

- [x] 1.1 Add `src/pullRequestWorkspace.ts` and `src/hooks/usePullRequestWorkspace.ts` for canonical per-surface identity, bounded versioned storage, selected tab and safe file-anchor restoration; handle unavailable storage and changed commits (`pr-review-workspace`: *PR Working Views and Resumption*).
- [x] 1.2 Refactor `src/components/PullRequestView.tsx` into accessible Overview, Files and Discussion panels under the persistent detail/action/editor owner, with correct counts and global freshness notices (`pull-request-viewer`: *Pull-Request View*).
- [x] 1.3 Wire lazy first mount and subsequent hidden-panel retention in `src/components/PullRequestView.tsx`, and visibility-aware measurement/restoration in `src/components/DiffView.tsx`; preserve loaded diffs, folds and per-view reading state without adding history entries or provider reads (`pr-review-workspace`: *PR Working Views and Resumption*).

## 2. Review-Aware File Navigation

- [x] 2.1 Add review filter predicates, unknown-state handling, per-file badges and ordered next-unreviewed selection in `src/pullRequestWorkspace.ts`, consuming existing progress and thread models (`pr-review-workspace`: *Review-Aware File Navigation*).
- [x] 2.2 Extend `src/components/DiffView.tsx` with optional generic navigator metadata, candidate filtering, current-file context and reveal-target methods; keep the complete file input and existing slot-cache/render-isolation behavior (`pr-review-workspace`: *Review-Aware File Navigation*).
- [x] 2.3 Wire path search, state filter, progress markers, clear-filter/empty states and Next unreviewed from `src/components/PullRequestView.tsx` into the diff navigator (`pr-review-workspace`: *Review-Aware File Navigation*).

## 3. Status and Discussion

- [x] 3.1 Implement readable status summaries and stable check grouping in `src/pullRequestWorkspace.ts` and `src/components/PullRequestView.tsx`; add Overview review-count details, collapsed passing checks and cross-view status jumps without inferring merge readiness (`pr-review-workspace`: *Actionable PR Status*).
- [x] 3.2 Add `src/components/PullRequestDiscussion.tsx` for general conversation and a separate All/Unresolved/Outdated thread index, preserving minimized/deleted content handling and accurate count labels (`pr-review-workspace`: *Discussion Index and Contextual Jumps*).
- [x] 3.3 Connect thread jumps and Next unresolved in `src/components/PullRequestView.tsx` and `src/components/DiffView.tsx` through existing `src/reviewAnchors.ts` classifications; reveal folded targets and handle preamble, unlisted and disappeared targets (`pr-review-workspace`: *Discussion Index and Contextual Jumps*).

## 4. Finish-Review Sheet

- [x] 4.1 Add `src/components/PullRequestReviewSheet.tsx` and the persistent native-only entry point in `src/components/PullRequestView.tsx`, reusing the mounted `ReviewDraftEditorProvider` and existing composer/status components (`pr-review-workspace`: *Persistent Finish-Review Sheet*).
- [x] 4.2 Connect draft-item index, progress summary and placed/unplaced item jumps in `src/components/PullRequestReviewSheet.tsx`; retain existing placement controls and do not create a second editor (`pr-review-workspace`: *Persistent Finish-Review Sheet*).
- [x] 4.3 Update `src/components/PullRequestReviewComposer.tsx` and sheet wiring to save on dismissal, retain unsaved state, expose save failures on the persistent action and show the byte indicator near the existing limit; preserve explicit native confirmation and all existing eligibility/recovery states (`pr-review-workspace`: *Persistent Finish-Review Sheet*).

## 5. Panel Triage and Responsive Presentation

- [x] 5.1 Pass canonical active PR identity from `src/App.tsx` into `src/components/PullRequestPanel.tsx` and `src/components/PullRequestControl.tsx`; add accessible selected-row styling and PR numbers while preserving row/worktree gestures (`pr-review-workspace`: *Provider-Panel Triage*).
- [x] 5.2 Add local search/filter controls, matching totals, clear-filter/no-match states and stable per-section refresh ordering in `src/components/PullRequestPanel.tsx` and presentation helpers in `src/pullRequestWorkspace.ts` (`pr-review-workspace`: *Provider-Panel Triage*).
- [x] 5.3 Refine identity/status/tab hierarchy and sticky clearances in `src/styles/base.css`; add a narrow file-list disclosure in `src/components/DiffView.tsx` with access to all navigator controls (`pr-review-workspace`: *Accessible and Responsive Review Workspace*).
- [x] 5.4 Complete keyboard tab semantics, hidden-panel exclusion, sheet focus containment/dismissal/restoration and explicit-jump destination focus in `src/components/PullRequestView.tsx`, `PullRequestReviewSheet.tsx`, `PullRequestDiscussion.tsx` and `DiffView.tsx` (`pr-review-workspace`: *Accessible and Responsive Review Workspace*).

- [x] 5.5 Correct mid-word fragmentation in split diffs in `src/styles/base.css` and detached-window scroll-port detection in `src/pullRequestWorkspace.ts`; verify the reported deeply indented TSX PR, oversized punctuation and detached tab resumption (`diff-view`: *Unified and Side-by-Side Layouts*; `pr-review-workspace`: *PR Working Views and Resumption*).
- [x] 5.6 Replace split source wrapping with synchronized code-only horizontal scrolling in `src/components/DiffView.tsx`, `src/hooks/useDiffHorizontalScroll.ts`, `src/diffHorizontalScroll.ts` and `src/styles/base.css`; verify unwrapped indented lines, fixed gutters/markers, shared scrollbar/trackpad interaction, retained tab position and unchanged copy/inline-thread behavior (`diff-view`: *Unified and Side-by-Side Layouts*).

## 6. Verification

- [x] 6.1 Add focused behavioral tests in `src/pullRequestWorkspace.test.ts` for storage fallback/eviction, canonical identity, unknown progress, filter intersections, next-target wraparound, status grouping and stable panel ordering.
- [x] 6.2 Extend `src/components/PullRequestView.test.tsx`, `PullRequestReviewComposer.test.tsx` and `PullRequestPanel.test.ts` with tab retention, stale/deleted target fallback, sheet save-failure persistence, canonical selection and no-extra-provider-read coverage; add `src/components/PullRequestReviewSheet.test.tsx` for native gating, shared drafts and confirmation delegation.
- [x] 6.3 Run the relevant `src/components/DiffView` and review-editor regression suites, adding behavioral coverage for optional navigator metadata, unchanged commit-host behavior and per-file render isolation; run the frontend suite with `bun test` and lint with `bun run lint`.
- [x] 6.4 Run `bun run build`, then `cargo test` with the fresh frontend bundle available, and resolve failures relevant to the change.
- [ ] 6.5 Run the app using `bun run wt:dev` and walk the change's spec scenarios in main/detached and narrow/wide views: keyboard tabs, sheet focus, pending/unplaced drafts, failed saves, post-push resumption, filtered current files, minimized/outdated threads, check links and macOS titlebar clearance. Use test/mocked submission paths for confirmation checks rather than posting incidental reviews.
- [x] 6.6 Run the served browser UI and verify working views, navigation, filtering and responsive layout while confirming no finish-review UI or native action reads; visually check a non-PR commit diff for regressions.
- [x] 6.7 Run `openspec validate pr-review-workspace --strict` and record verification results before marking the implementation complete.

### Verification Results — 2026-11-16

- `bun test`: 1,478 passed, zero failures, including horizontal-range/synchronization, fixed-marker/slot structure, unchanged unified presentation and existing per-file render-isolation coverage. `bun run lint` and `bun run build` passed.
- `cargo test`: full workspace passed again after the final unwrapped-diff frontend build.
- `openspec validate pr-review-workspace --strict` and `git diff --check` passed.
- Live served UI inspected against GitHub PR #23; main/detached and 480px/1560px presentations checked. Browser-native-mode fixtures used mocked commands only, never provider review writes.
- Browser checks passed for retained diff DOM/scroll across tabs, filters retaining sections, outdated/unlisted anchors, hidden minimized excerpts, unplaced draft navigation, failed-save retention, Escape/focus restoration, modal Tab containment, narrow navigator disclosure, saved-position resumption and cached-then-fresh commit fallback.
- The earlier word-boundary check was insufficient: source indentation still caused zigzag continuations. The stronger whole-line check reproduced 119 wrapped source cells out of 312 in the reported PR before the no-wrap change.
- Final `scripts/check-diff-wrap.js` result for that PR: 312 source cells, zero wraps, three overflowing files, equal synchronized horizontal ranges through their maximum offsets, stationary gutters/markers/headers, and aligned paired rows.
- Actual browser interaction passed for horizontal wheel input on both sides, mouse dragging the shared scrollbar and keyboard scrolling. These preserved the detached pane's vertical position at 719px; switching Discussion → Files retained the final 532px horizontal offset. Scrollbar dragging was checked with Playwright's default scrollbar-hiding launch flag disabled.
- Increasing the code font to 16px, restoring it, resizing to 1440px, falling back to unified at 480px and returning to 1560px all passed. Copying the reported indented source line preserved its exact source whitespace.
- A main-pane native-mode fixture with an inline thread and draft passed the same geometry check for 150 source cells. Copying three lines across both slots produced exactly 750 source characters without discussion text; an open draft editor retained its DOM and body during horizontal scrolling. No provider submissions were made.
- Non-PR commit host inspected with its normal navigator and split layout, without PR controls. After the no-wrap change, the standalone shared renderer was visually rechecked without PR metadata; its 200 source cells passed the same no-wrap, range and fixed-context checks.
- Task 6.5 remains incomplete: `bun run wt:dev` launched the native worktree app, but desktop-control tools are unavailable in this session. Shared native-mode UI is Chromium-browser-tested; native macOS titlebar/drag-region interaction still needs an on-screen smoke check. A separate WebKit browser run was unavailable because the Playwright WebKit executable is not installed; no browser installation was performed.
