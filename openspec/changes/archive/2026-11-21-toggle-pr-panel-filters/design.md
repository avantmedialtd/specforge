## Context

See `proposal.md` for motivation. The committed baseline renders the status selector whenever either provider panel lists rows. Both providers and both React hosts share `src/components/PullRequestPanel.tsx`; filtering already runs entirely over the supplied snapshot.

This change was scaffolded after the implementation had already been written in this worktree. It corrects that workflow mistake without claiming the artifacts preceded the code. The direct main-spec edit has been removed; the delta is held in this change for the normal sync/archive boundary. Tasks must be checked against the existing implementation and actual verification results.

## Goals / Non-Goals

**Goals**

- Make hidden controls and an active filter mutually exclusive by construction.
- Keep disclosure behavior independent from panel collapse and from other mounted panels.
- Fit the existing compact header while retaining keyboard, narrow-panel and coarse-pointer access.

**Non-Goals**

- Persisting filter visibility, changing application settings, or adding service commands.
- Changing provider integration enablement, snapshot reads, PR navigation or the Files navigator.

## Decisions

### One local state represents both disclosure and the selected filter

Use `PanelFilter | null`: `null` means controls hidden and an effective All filter; a non-null value means controls visible. Toggling on selects `"all"`; toggling off selects `null`. Clear continues to select `"all"` without hiding the controls. The state transitions are shown in the proposal.

Rejected: retaining a selected status while merely hiding the dropdown, because that leaves rows silently filtered. A separate visibility flag and filter state also permits that inconsistent state and needs extra reset coordination.

Rejected: a persisted application setting or local-storage preference, because this is temporary panel view state and the requested default is off on mounting. No storage migration or IPC change is needed.

### The disclosure and collapse controls are siblings

Wrap the existing collapse button and a funnel-icon button in one flex heading. Render the filter toggle only while an expanded panel lists rows. The toggle has a provider-specific accessible label, `aria-expanded`, a generated controls id while visible, Show/Hide filters tooltips and a visible focus treatment.

Rejected: placing the toggle inside the collapse button, which nests interactive controls and couples two different actions. A separate full-width controls row was also rejected because the default view would still pay the vertical-space cost.

### Keep one shared behavior for both providers and hosts

Implement in the existing panel component, preserving GitHub-only Review requested and all existing row/section decisions. Styles use theme tokens, keep the heading label shrinkable and reserve a 44×44 target under coarse-pointer input.

Rejected: a GitHub-only branch or a host-specific setting, which would make the same shared triage controls behave differently without a provider or transport reason.

### Split the inherited triage contract without losing behavior

The user explicitly approved restructuring the specification after strict validation rejected the existing requirement-length warning. Keep *Provider-Panel Triage* as a concise preservation contract and retain every scenario under it. Add separate focused requirements for status-filter semantics, optional controls and their local lifecycle, toggle accessibility, filter feedback and stable row order. Each requirement description stays within the validator's 500-character limit; the scenarios preserve the detail.

Rejected: accepting a validation exception, because the user chose to make the strict gate pass. Trimming old behavior or dropping scenarios is also rejected: the split is a specification-structure change, not permission to narrow the feature contract. Main specs remain untouched until sync/archive.

## Risks / Trade-offs

- Hiding loses the selected status → This is the explicit reset-on-hide contract; tests cover hiding a no-match filter, restoring all rows and reopening at All.
- Remounting hides controls again → This preserves the off-by-default behavior; tests cover remounts and independence between providers.
- The extra header control reduces title space → Keep the collapse button shrinkable, retain title ellipsis, and verify a narrow panel plus the coarse-pointer target in a running browser.
- Existing triage behavior could regress during the header refactor → Retain coverage for every status filter, Clear, snapshot totals, no-match wording, stable ordering, active selection and no additional transport calls.
