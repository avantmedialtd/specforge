# Hide quiet workspaces in the Today's-commits garden

## Why

The commit garden ("Today's commits") renders a full bordered card for **every**
registered workspace, including those with no commits today — each showing a
"quiet today" placeholder. On a typical day most workspaces are quiet, so the
genuinely-active plots (the point of the section) are buried among empty boxes.
The garden also sits flush against the analytics grid above it, with no
breathing room beneath its title.

## What Changes

- The commit garden hides any workspace with no commits on the current local day
  (the dormant entries — quiet repositories, non-git workspaces, and the
  git-unavailable case all collapse to this), rendering only the workspaces that
  actually moved today.
- When **every** registered workspace is quiet, the whole "Today's commits"
  section is omitted (consistent with the existing empty-registry rule) rather
  than showing a section of empty cards or a lonely header.
- The "Today's commits" section gains top margin so its title no longer hugs the
  analytics overview above it.
- Because the garden no longer keeps a per-workspace dormant placeholder, the
  dashboard's *Today's Ships Quiet State* requirement drops its claim to mirror
  "the commit garden's dormant treatment"; the today's-ships quiet-day note now
  stands on its own rationale.

## Capabilities

### Modified Capabilities

- `commit-garden`: dormant/quiet entries are **omitted** from the section
  instead of rendering a "quiet today" placeholder; when all entries are quiet
  the section is omitted entirely.
- `dashboard`: the *Today's Ships Quiet State* requirement no longer ties its
  quiet-day note to the commit garden's (now removed) dormant treatment.

## Impact

- `src/components/CommitGarden.tsx` — filter the plant list to active plots;
  return `null` when none remain; the dead dormant render branch can be dropped.
- `src/App.css` — top margin on `.dashboard-garden-section`; the now-unused
  `.garden-plot--dormant` rules can be removed.
- No Rust / IPC change: the backend still emits per-workspace gardens with the
  `dormant` flag; the filtering is presentational, keeping the flag available for
  a possible future "show quiet workspaces" toggle.
