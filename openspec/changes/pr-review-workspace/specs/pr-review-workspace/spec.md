## ADDED Requirements

### Requirement: PR Working Views and Resumption

The PR workspace SHALL provide Overview, Files and Discussion tabs in both main and detached presentations. Overview SHALL contain description, checks and review-count summary; Files SHALL contain the shared diff; Discussion SHALL contain general conversation and the thread index. Files SHALL show the total changed-file count including explicitly unlisted files. Discussion SHALL count general conversation entries plus thread roots, not replies, and label unresolved counts separately.

First visit SHALL open Overview. Tab choice SHALL persist per canonical PR identity and viewer surface in a bounded record of 50 recent PRs, with memory fallback if storage is unavailable. Switching tabs SHALL preserve mounted diff loading, folding, editor and per-tab reading state. Reopening SHALL restore the selected tab and file location when available, falling back to the file header or Files start when the saved anchor cannot be used. Changed head/base commits SHALL invalidate saved line-level positions. Tabs SHALL NOT create application history entries or additional provider reads. Explicit jumps SHALL override tab resumption. Window-local tab/filter/scroll changes SHALL NOT move another window.

#### Scenario: Returning to files within a review
- **WHEN** a reviewer switches from a loaded, folded Files view to Discussion and back
- **THEN** loaded diffs, folds, draft content and reading position are retained
- **AND** hidden panels offer no focusable or accessibility-visible controls

#### Scenario: Reopening after a push
- **WHEN** a PR is reopened in its remembered Files view with different commits
- **THEN** the saved file is restored if still present, without restoring an obsolete line position
- **AND** a removed file falls back to the start of Files

#### Scenario: Storage is unavailable
- **WHEN** presentation-state storage cannot be read or written
- **THEN** navigation still works with memory-local state and no provider request is added

### Requirement: Review-Aware File Navigation

The Files navigator SHALL offer case-insensitive path search intersected with one filter: All, Unreviewed, Changed since review, Has discussion or Skipped. Unreviewed SHALL mean included files not viewed at the current patch; Has discussion SHALL include files with any listed review thread. Existing review progress SHALL determine viewed, changed and skipped states. State markers SHALL use text or accessible labels as well as color.

Search and filters SHALL narrow navigator candidates and preserve folder ancestry, without removing rendered file sections, active editors, drafts or cached loads. A current file that stops matching SHALL remain explicitly identifiable until the reader navigates elsewhere. Progress-dependent filters and Next unreviewed SHALL be unavailable with an explanation while progress is unknown. Next unreviewed SHALL traverse matching candidates in file order, wrap once, exclude viewed and skipped files, and disable with an explanation when none remain. Filtered empty results SHALL offer a clear-filter control.

#### Scenario: Marking the current filtered file viewed
- **WHEN** the reader marks the current file viewed under Unreviewed
- **THEN** the mark follows existing completion/folding behavior without removing the file section or losing an editor
- **AND** the current-file context remains identifiable and Next unreviewed targets another eligible file

#### Scenario: Unknown review progress
- **WHEN** review progress is unavailable
- **THEN** no file is asserted to be unreviewed or skipped based on missing data
- **AND** All, path search and Has discussion remain usable

#### Scenario: No matching remaining file
- **WHEN** search and the selected filter have no unreviewed included candidates
- **THEN** Next unreviewed is disabled with an explanation and the reader can clear filtering

### Requirement: Persistent Finish-Review Sheet

Native PR presentations SHALL expose a Finish review control while reading any working view, labelled with the pending draft-item count. It SHALL open a sheet with pending comments/replies, local viewed/remaining/skipped progress, summary, explicit decision, existing action status and Submit. Loading or unavailable action state SHALL remain inspectable through the sheet. The browser SHALL render neither control nor sheet and SHALL NOT read native draft/action state.

The sheet SHALL share the existing draft editor with inline controls. Opening or closing SHALL NOT create, discard or submit a draft; closing SHALL invoke the existing save path, retain unsaved editor state and leave save failures discoverable from the persistent control. Draft-item navigation SHALL reveal the corresponding Files anchor; unplaced items SHALL lead to existing placement controls without inventing a location. Remaining files SHALL be informational and SHALL NOT add a submission eligibility restriction. Submission SHALL retain the *Native Confirmation Freezes One Exact Draft Revision* requirement in the `pull-request-actions` capability. The byte limit SHALL remain enforced, with its indicator shown at 90% of the limit or on validation failure.

#### Scenario: Finishing from a deep diff
- **WHEN** a native reviewer opens Finish review while reading a diff far below its beginning
- **THEN** the sheet shows the same draft as inline editors without requiring a scroll to Overview
- **AND** submitting uses the existing native confirmation flow

#### Scenario: Closing while a save fails
- **WHEN** the sheet is dismissed and saving the edited summary fails
- **THEN** the unsaved editor content survives reopening and the failure remains discoverable outside the closed sheet

#### Scenario: An unplaced draft item
- **WHEN** the reviewer activates an unplaced draft item in the sheet
- **THEN** the sheet closes and Files reveals its existing placement controls
- **AND** no line is automatically assigned

#### Scenario: Browser presentation
- **WHEN** the PR is opened in the browser skin
- **THEN** the read-only working views are available without a finish-review control, sheet or native draft read

### Requirement: Actionable PR Status

The detail header SHALL show readable review, check and unresolved-discussion summaries with explicit unknown states and no inferred merge-readiness claim. Loaded check counts SHALL derive from the displayed detail and identify their last-read nature; before detail loads, row signals SHALL remain the fallback. Clicking checks SHALL reveal Overview checks; clicking reviews SHALL reveal the Overview review-count summary; clicking unresolved discussions SHALL reveal the Unresolved thread index.

Checks SHALL retain all states and safe provider links, order failure and pending groups before other non-passing states and passing checks, preserve original order within groups, and collapse the passing group by default. Existing stale/deferred notices SHALL remain visible independently of the selected tab.

#### Scenario: Inspecting a failed check
- **WHEN** the reviewer activates the failing-check summary from Files
- **THEN** Overview opens at checks with failed checks visible and passing checks collapsed
- **AND** each safe Details link retains the existing provider-link behavior

#### Scenario: Unknown review counts
- **WHEN** the provider row has no review summary
- **THEN** the header and Overview state that review status is unknown rather than showing zero approvals

### Requirement: Discussion Index and Contextual Jumps

Discussion SHALL preserve general conversation submission order and provide a separate thread index filtered by All, Unresolved or Outdated. Each entry SHALL identify path, available location, author, excerpt, reply count and resolved/outdated state. Minimized and deleted comment content SHALL retain existing hiding/deletion behavior in excerpts. Thread filtering SHALL NOT filter general conversation.

Thread jumps SHALL switch to Files and reveal the existing side-qualified anchor, expanding the necessary file/hunk. Outdated, ranged or unavailable line locations SHALL use the existing file preamble; unlisted paths SHALL use the unlisted-thread section. Jumps SHALL reveal out-of-filter targets without changing filter settings. If a refresh removes the target, the UI SHALL explain its absence and retain access to the index. Next unresolved SHALL follow file order then unlisted-thread order, wrap once, and disable when none exist.

#### Scenario: Jump to a folded thread
- **WHEN** an unresolved thread belongs to a folded file/hunk outside the current navigator filter
- **THEN** its jump reveals the thread in Files with the needed folds opened
- **AND** navigator filter settings remain unchanged

#### Scenario: Outdated or unlisted location
- **WHEN** the selected thread has an outdated line or an unlisted path
- **THEN** navigation reveals its preamble or unlisted-thread presentation rather than a guessed line

#### Scenario: Minimized content in the index
- **WHEN** a thread's initial comment is minimized by the provider
- **THEN** its index entry retains the minimized reason without exposing the hidden body as an excerpt

### Requirement: Provider-Panel Triage

Existing provider panels SHALL retain their provider scopes, sections, counts, collapse controls and open/worktree gestures, and add PR numbers, canonical active-PR highlighting and local search across title, repository, number and branch names. Search SHALL be case-insensitive and intersect one filter: All, Failing checks, Conflicts or Drafts, plus Review requested for GitHub only.

Header totals SHALL remain snapshot totals; active filtering SHALL show matching/total counts and a clear-filter affordance. No-match results SHALL be distinct from an empty provider list or authentication failure. Search and filters SHALL NOT trigger provider reads. Initial row order SHALL follow the snapshot; subsequent snapshots SHALL preserve surviving row order within each section, append newcomers in snapshot order and remove missing rows without remounting a focused surviving row.

#### Scenario: Finding a requested review
- **WHEN** the reader selects GitHub Review requested and searches a branch name
- **THEN** only matching requested-review rows are listed with matching/total counts
- **AND** no extra provider request occurs

#### Scenario: A refresh changes urgency
- **WHEN** a surviving row changes from passing to failing checks during a panel refresh
- **THEN** its status updates without moving relative to other surviving rows in its section
- **AND** a focused row retains focus

#### Scenario: Canonical active selection
- **WHEN** the current PR address uses different casing from its panel row
- **THEN** that row remains visibly and accessibly marked as the current PR

### Requirement: Accessible and Responsive Review Workspace

Tabs SHALL expose selected state, associated panels and conventional keyboard navigation. Hidden panels SHALL be excluded from keyboard and assistive-technology navigation. The review sheet SHALL have an accessible name, contain focus while open, support Escape dismissal and restore focus to its invoking control or the explicit navigation destination.

At narrow widths the navigator SHALL remain reachable through a disclosure containing search, filters and jump controls. All essential controls SHALL be reachable without hover, preserve existing coarse-pointer targets and macOS titlebar clearance, and keep sticky file headings below the complete workspace header. Non-PR diff hosts SHALL retain their existing behavior when optional PR navigation metadata is absent.

#### Scenario: Keyboard-only review
- **WHEN** a reviewer changes tabs and opens then dismisses Finish review using the keyboard
- **THEN** selection and focus are announced correctly, hidden panels are skipped, and dismissal restores focus

#### Scenario: Narrow detached view
- **WHEN** a detached PR window is too narrow for the file navigator beside the diff
- **THEN** file search, filters and navigation remain available through the disclosure
- **AND** the diff retains its existing responsive layout fallback
