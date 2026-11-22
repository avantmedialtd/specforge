## 1. Selection and Navigation

- [x] 1.1 Extend `src/pullRequestWorkspace.ts` traversal with direction and test wrap, viewed/skipped exclusion, filtering and empty targets in `src/pullRequestWorkspace.test.ts` (`pr-review-workspace`: *Bidirectional Unreviewed File Controls*).
- [x] 1.2 Add controlled file visibility and selection-aware tree/reveal behavior in `src/components/DiffView.tsx`, retaining loads, folds and editors; cover visibility, late results and unchanged default hosts in `src/components/DiffView.test.tsx` (`diff-view`: *Host-Selected File Visibility*).
- [x] 1.3 Wire selection, Previous/Next and position in `src/components/PullRequestView.tsx`; adapt `src/hooks/usePullRequestWorkspace.ts` resumption and capture; add regression coverage in `src/components/PullRequestView.test.tsx`, style controls in `src/styles/base.css` and document the contract in `src/CLAUDE.md` (`pr-review-workspace`: *Single-File Review Presentation*, *Bidirectional Unreviewed File Controls*).

## 2. Verification

- [x] 2.1 Run `bun run lint`, `bun test`, `bun run build` and `openspec validate single-file-review-navigation --strict`; resolve new failures.
- [x] 2.2 Run `cargo test --workspace` after the frontend build to verify integration.
- [x] 2.3 Run the app and visually smoke-test single-file selection, both navigation directions, viewed/skipped exclusion, all-reviewed and unknown progress, filtering, resumption and discussion jumps at wide and narrow widths.

Rust integration passed with `BITBUCKET_USERNAME`, `BITBUCKET_API_TOKEN`, `GH_TOKEN` and `GITHUB_TOKEN` removed from the test process environment. Inherited credentials otherwise override the hermetic credential fixtures in two existing tests. No Rust code changed.

Frontend verification: lint, all 1,589 Bun tests, strict typecheck/build and strict change validation passed. Browser smoke used this worktree's Vite server on port 1430 with the existing real backend, in main and detached presentations at 1,167/1,280px and 620/480px. Verified only one visible section, both traversal directions and wrapping, filtered navigation and clearing, saved-file resumption, and a focused discussion jump. Viewed/all-viewed/unavailable progress and the discussion anchor were simulated in browser-only fetch responses; no actual review marks or provider content were written.

Review follow-up: restored scrolling when the current tree entry is reactivated. Regression tests cover repeated pointer, Enter and Space activation with sticky-header offset and retained drafts. Browser verification of the shared renderer confirmed all three return to the current file header, while selecting another file still positions the toolbar.
