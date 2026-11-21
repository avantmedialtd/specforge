## 1. Frontend

- [x] 1.1 In `src/components/PullRequestPanel.tsx`, represent hidden controls as a null local filter, initially hidden, and restore All whenever the controls are hidden or reopened. Verify default-hidden rendering, absence of an empty controls row and reset from a no-match filter in `src/components/PullRequestPanel.test.ts` (`pr-review-workspace`: *Provider-Panel Triage*).
- [x] 1.2 In `src/components/PullRequestPanel.tsx`, `src/components/icons.tsx` and `src/styles/base.css`, add a funnel toggle beside, not inside, the collapse button, with provider-specific labelling, expanded/controls attributes, focus styling, a shrinkable heading and a 44×44 coarse-pointer target. Verify disclosure attributes in `src/components/PullRequestPanel.test.ts` and keyboard/touch behavior in the running UI (`pr-review-workspace`: *Provider-Panel Triage*).
- [x] 1.3 In `src/components/PullRequestPanel.test.ts`, cover independent provider toggles, remounting with filters off and the retained status options, matching counts, Clear, header totals, no-match wording, stable row/focus behavior, active selection and absence of additional transport calls. Run `bun test src/components/PullRequestPanel.test.ts src/pullRequestWorkspace.test.ts` (`pr-review-workspace`: *Provider-Panel Triage*).
- [x] 1.4 With the user's approved scope expansion, restructure `openspec/changes/toggle-pr-panel-filters/specs/pr-review-workspace/spec.md` into a concise MODIFIED *Provider-Panel Triage* requirement retaining every scenario and focused ADDED requirements for the extracted behavior. Update this change's `proposal.md` and `design.md`; verify no behavior or original scenario is lost, each description is at most 500 characters and strict OpenSpec validation passes.

## 2. Verification

- [x] 2.1 Run `bun test` and `bun run lint` across the frontend after the changes to `src/components/PullRequestPanel.tsx`, `src/components/icons.tsx`, `src/styles/base.css` and `src/components/PullRequestPanel.test.ts`; confirm both pass.
- [x] 2.2 Run `bun install --frozen-lockfile` and `bun run build`; confirm the frontend typecheck and production build pass and `dist/` exists for Rust compilation.
- [x] 2.3 Run `cargo test` for the workspace after the frontend build; confirm the unchanged Rust frontends and services remain green.
- [x] 2.4 Smoke-test `src/components/PullRequestPanel.tsx` through `bun run wt:dev` or the repository's lighter `specforge-serve` + Vite browser loop. Walk the delta's default-hidden, reveal, status-filter, hide/reset, reload, keyboard, narrow-panel and coarse-pointer scenarios; use the component tests for provider independence and snapshot-refresh cases (`pr-review-workspace`: *Provider-Panel Triage*).
- [x] 2.5 Run `openspec validate toggle-pr-panel-filters --strict` and `git diff --check`. Confirm `openspec/changes/toggle-pr-panel-filters/` contains the proposal, design, complete modified-requirement delta and tasks, and `openspec/specs/pr-review-workspace/spec.md` is unchanged pending normal sync/archive.

## Workflow follow-up

- Checklist progress records task-by-task reconciliation and rerun checks during apply, rather than assuming the earlier implementation completed the plan.
- The Rust suite passed using `env -u GH_TOKEN -u GITHUB_TOKEN -u BITBUCKET_USERNAME -u BITBUCKET_API_TOKEN cargo test`, with user-approved subprocess-only environment isolation after two existing credential tests read shell overrides. No Rust files changed and no tests were skipped.
- The user approved splitting the inherited long triage contract to satisfy strict validation rather than accepting a warning exception. Preserve every scenario and extract each behavior into focused requirements without modifying main specs directly.
- Review and verify the completed change before archiving.
- Sync the delta into main specs through the normal OpenSpec sync/archive workflow, not by editing the main requirement directly.
