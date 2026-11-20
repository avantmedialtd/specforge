# Tasks

`design.md` is intentionally omitted: this is a bounded frontend removal with no architectural, migration or security decisions.

## 1. Frontend

- [x] 1.1 Remove widget query state, search input and text matching from `src/components/PullRequestPanel.tsx` and `src/pullRequestWorkspace.ts`, retaining status filters and Clear (`pr-review-workspace`: *Provider-Panel Triage*); verify all matcher call sites use filter-only arguments and focused tests pass.
- [x] 1.2 Update `src/components/PullRequestPanel.test.ts` and `src/pullRequestWorkspace.test.ts` with regression coverage for both providers' absent search fields, all retained filters, snapshot totals, matching counts, no matches and Clear (`pr-review-workspace`: *Provider-Panel Triage*); verify with `bun test src/components/PullRequestPanel.test.ts src/pullRequestWorkspace.test.ts`.
- [x] 1.3 Remove the unused panel-input selector from `src/styles/base.css`, preserving file-search and shared select styling (`pr-review-workspace`: *Provider-Panel Triage*); verify the production build and visual smoke show no blank search row or clipped controls.

## 2. Verification

- [x] 2.1 Run `bun test`, `bun run lint` and `bun run build` from the worktree and verify all frontend checks pass, including existing Files navigator search tests.
- [x] 2.2 Run `cargo test` after the frontend bundle exists and verify the unchanged Rust workspace remains green.
- [x] 2.3 Start the app and walk the provider-panel scenarios at normal and narrow widths, using the browser loop (`specforge-serve` plus `bun run dev`) or `bun run wt:dev`; verify both providers have no search row, filters/counts/Clear work locally, row gestures remain intact and Files search is unchanged.
- [x] 2.4 Review the code and scenario coverage against `openspec/changes/remove-pr-widget-search/specs/pr-review-workspace/spec.md`, run `openspec validate remove-pr-widget-search --strict`, and verify no unrelated behavior or source files changed.

Verification evidence:

- `bun test`: 1,559 passing tests; `bun run lint`, `bun run build` and `git diff --check` pass.
- The Rust workspace suite passes with `BITBUCKET_USERNAME`, `BITBUCKET_API_TOKEN`, `GH_TOKEN` and `GITHUB_TOKEN` unset for the test process so exported credentials do not override fixture credentials.
- Browser smoke used the running worktree app with tab-local fixture snapshots at 1280×900 and 700×800. Both widgets omit search without a blank row; filters, counts, no matches and Clear work without provider reads. Normal row opening and Cmd-click opening preserve their existing behavior. Files search still matches `ALPHA` to `src/alpha.ts` without a detail read.
- Regression tests cover stable row order and focused-row reuse when a refresh changes urgency, plus canonical active-PR marking.
- Normal OpenSpec validation passes. Strict validation reports only the existing over-500-character *Provider-Panel Triage* requirement warning; the baseline requirement already has that warning, and the schema requires preserving the full modified requirement rather than splitting it in this change.

## Workflow follow-up

- Sync the completed delta into the main `pr-review-workspace` spec and archive the change.
- Verify the archived proposal, delta and completed tasks remain present.
