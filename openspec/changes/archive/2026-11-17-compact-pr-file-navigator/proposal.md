# Compact PR File Navigator

## Why

The PR file navigator currently spends two or three lines on each file: the filename and change counts can wrap, and a full-width review-status label always occupies another line. On large pull requests this hides useful files below repeated metadata and makes the navigator slower to scan.

## What Changes

- Keep every PR file entry on one visual line, with its change-status letter, truncating filename, inline added/removed counts and compact review-state icon.
- Distinguish viewed, unreviewed, partly viewed, changed-since-review, skipped and unknown states by icon shape, with complete words in the row's accessible name and tooltip. Add a small discussion indicator only when the file has threads; retain its exact count in the accessible description.
- Tighten the controls to a search field followed by a compact filter, matching count and Next unreviewed row. Preserve visible exceptional notices and the conditional clear-filter action.
- Preserve the existing folder tree, compacted directory chains, full visible indentation, keyboard navigation, active-file tracking, filtering and review semantics. Deep trees scroll inside their own viewport rather than flattening the hierarchy; search and filters stay fixed.
- Keep the current navigator width, typography, theme tokens and narrow-view disclosure rather than recovering space by shrinking text or widening the sidebar.

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="360" height="220" viewBox="0 0 360 220">
  <rect x="1" y="1" width="358" height="218" rx="5" fill="#13171d" stroke="#596372"/>
  <g fill="#e4e8ee" font-family="monospace" font-size="12">
    <rect x="12" y="12" width="336" height="28" rx="3" fill="none" stroke="#596372"/>
    <text x="22" y="30">Find file…</text>
    <rect x="12" y="48" width="145" height="25" rx="3" fill="none" stroke="#596372"/>
    <text x="22" y="65">All</text>
    <text x="171" y="65">162 files</text>
    <rect x="259" y="48" width="89" height="25" rx="3" fill="none" stroke="#596372"/>
    <text x="271" y="65">Next →</text>
    <path d="M12 84H348" stroke="#596372"/>
    <text x="13" y="106">M .oxlintrc.json</text>
    <text x="251" y="106" fill="#4edbad">+6</text>
    <text x="278" y="106" fill="#fa8585">−3</text>
    <circle cx="336" cy="102" r="5" fill="none" stroke="#b6bfcd"/>
    <path d="M15 120l4 4 4-4" fill="none" stroke="#b6bfcd"/>
    <text x="31" y="129">npm</text>
    <text x="25" y="154">M build-packages.mjs</text>
    <text x="251" y="154" fill="#4edbad">+5</text>
    <text x="278" y="154" fill="#fa8585">−5</text>
    <circle cx="336" cy="150" r="5" fill="none" stroke="#b6bfcd"/>
    <text x="25" y="178">M publish.mjs</text>
    <text x="251" y="178" fill="#4edbad">+3</text>
    <text x="278" y="178" fill="#fa8585">−3</text>
    <path d="M331 174l4 4 7-8" fill="none" stroke="#4edbad" stroke-width="1.5"/>
    <text x="25" y="202">M resolve.mjs</text>
    <text x="251" y="202" fill="#4edbad">+1</text>
    <text x="278" y="202" fill="#fa8585">−1</text>
    <path d="M331 198h10" stroke="#b6bfcd" stroke-width="1.5"/>
  </g>
</svg>
```

The sketch illustrates the layout, not a new theme or sidebar width.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `pr-review-workspace`: Make review-aware file navigation compact and single-line, preserving complete accessible metadata, honest unknown states, existing controls and navigation behavior.

## Impact

- Presentation and metadata: `src/components/DiffView.tsx`, `src/components/PullRequestView.tsx`, `src/pullRequestWorkspace.ts`, `src/diffFiles.ts`, `src/components/icons.tsx` and PR-scoped rules in `src/styles/base.css`.
- Tests: `src/pullRequestWorkspace.test.ts`, `src/diffFiles.test.ts`, `src/components/DiffView.test.tsx`, `src/components/PullRequestView.test.tsx` and a focused real-browser geometry check in `scripts/check-pr-file-navigator.js`.
- Frontend only: no Rust, IPC types, provider API calls, polling, permissions, settings, persistence format or dependency changes.
- No changes to diff source rendering, synchronized scrolling, file headers, draft editors, review submission, viewed-mark writes or Files-tab progress. Non-PR diff hosts retain their existing presentation and behavior.
