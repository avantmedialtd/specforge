# pr-review-workspace Specification

## Purpose

Defines the pull-request review workspace across main and detached presentations: resumable working views and Files-tab progress, review-aware file navigation, a native finish-review sheet, actionable status and discussion jumps, local provider-panel triage, and accessible responsive controls without additional provider reads.

## Requirements

### Requirement: PR Working Views and Resumption

The PR workspace SHALL provide Overview, Files and Discussion tabs in both main and detached presentations. Overview SHALL contain description, checks and review-count summary; Files SHALL contain the shared diff; Discussion SHALL contain general conversation and the thread index. Files SHALL show the total changed-file count including explicitly unlisted files. Discussion SHALL count general conversation entries plus thread roots, not replies, and label unresolved counts separately.

Beside its total count, the Files tab SHALL show local viewed-file progress using the same themed meter and completion mark as the Tasks tab. Its denominator SHALL exclude skipped files. The accessible tab name and tooltip SHALL give exact viewed/eligible counts, skipped and changed-since-viewed counts when present, and explicitly unlisted files. Unavailable progress, progress for different commits or a different listed-file count, an empty review and an all-skipped review SHALL NOT show a progress meter or completion mark. A completion mark SHALL require at least one eligible file, every eligible file viewed and no explicitly unlisted files. This presentation SHALL consume the existing progress read and add no requests.

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

#### Scenario: Files tab shows review progress
- **WHEN** a PR has ten listed files, three skipped files and four viewed files
- **THEN** Files retains its total count of ten and shows a four-of-seven progress meter
- **AND** the tab's accessible name and tooltip state the exact counts, without counting skipped files as viewed

#### Scenario: Files tab completion stays honest
- **WHEN** every eligible file is viewed and no file is explicitly unlisted
- **THEN** the Files tab replaces its progress meter with the existing completion mark
- **AND** unavailable or stale progress, no eligible files, or explicitly unlisted files never produce that completion mark

### Requirement: Review-Aware File Navigation

The Files navigator SHALL offer case-insensitive path search intersected with one filter: All, Unreviewed, Changed since review, Has discussion or Skipped. Unreviewed SHALL mean included files not viewed at the current patch; Has discussion SHALL include files with any listed review thread. Existing review progress SHALL determine viewed, changed and skipped states. State markers SHALL use text or accessible labels as well as color.

Search and filters SHALL narrow navigator candidates and preserve folder ancestry, without removing rendered file sections, active editors, drafts or cached loads. A current file that stops matching SHALL remain explicitly identifiable until the reader navigates elsewhere. Progress-dependent filters and Next unreviewed SHALL be unavailable with an explanation while progress is unknown. Next unreviewed SHALL traverse matching candidates in file order, wrap once, exclude viewed and skipped files, and disable with an explanation when none remain. Filtered empty results SHALL offer a clear-filter control.

Each PR file entry SHALL occupy one visual line, containing its change-status letter, filename, added/removed counts where supplied, a compact review-state icon and a discussion-presence icon when threads exist. Review-state words and discussion counts SHALL NOT occupy additional lines below the file. Counts SHALL remain exact, retain their addition/removal signs and remain on the filename's line. Unavailable counts SHALL remain absent, not become zero. Long filenames and directory labels SHALL truncate with ellipsis instead of moving metadata onto another line. File-row height SHALL NOT vary because of filename length, review state or discussion count. When hierarchy depth and inline metadata need more width, horizontal scrolling SHALL be confined to the file tree rather than widening the sidebar or scrolling its controls.

Compact rows SHALL retain the existing text size and sidebar width. Every visible parent/child nesting level SHALL have a corresponding additional indentation step; visual depth SHALL NOT be capped while semantic levels continue increasing. Logical directory ancestry, semantic levels, compacted directory chains and keyboard parent/child navigation SHALL remain intact. The tree SHALL retain a usable filename reading width and scroll inside its own bounded viewport when needed, leaving search, filter, matching count, Next and exceptional notices fixed outside both scroll axes. Keyboard navigation SHALL reveal the focused filename within that viewport. Text scaling and existing coarse-pointer target sizes SHALL remain supported without clipping text or introducing a second metadata line.

Review icons SHALL distinguish unviewed, viewed, partly viewed, changed since review, skipped and unknown states by shape, not color alone. Missing or stale progress and missing per-file progress SHALL use the unknown state, never an inferred unviewed or skipped state. The complete safely displayed path, change status, available added/removed counts, review-state words and exact nonzero discussion count SHALL be available in each file row's tooltip and accessible name. Decorative icons SHALL NOT duplicate these announcements, add tab stops or act as review controls. Activating a row or one of its icons SHALL retain file navigation behavior and SHALL NOT mark the file viewed or change its inclusion.

The normal controls area SHALL present a search field followed by one compact toolbar row containing the filter selector, exact matching-file count and Next unreviewed control. Search and filter controls SHALL retain programmatic labels; a shorter visible Next label SHALL retain the full accessible name Next unreviewed. Unknown-progress, empty-result, out-of-filter-current-file and disabled-action explanations SHALL remain discoverable. Clear filters SHALL remain available when filtering is active. Exceptional notices and existing target-size requirements SHALL be allowed additional controls-area height rather than hiding information to enforce a fixed total height.

The compact presentation SHALL apply in main and detached PR views, in desktop and browser hosts and inside the existing narrow-view disclosure. It SHALL preserve the *File Navigator* requirement in the `diff-view` capability, including the single roving Tab stop, current-file marker and scroll/activation behavior. Supplying no optional PR navigation metadata SHALL preserve non-PR diff-host presentation and behavior. The presentation SHALL consume existing detail, progress and thread data without adding provider reads, review writes, settings or persistence.

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

#### Scenario: Long nested filenames remain one row
- **WHEN** a PR file with a long nested path, 1,234 added lines, 5,678 removed lines and three discussion threads appears at the existing sidebar width
- **THEN** its filename truncates while its change-status letter, exact signed counts, review icon and discussion icon remain on the same line
- **AND** its height matches an otherwise equivalent short-name file row, no second metadata line appears and any necessary horizontal scrolling stays inside the tree while the sidebar and controls retain their width

#### Scenario: Deep folders remain visibly nested
- **WHEN** a tree contains parent/child directories and files nested beyond three levels
- **THEN** every deeper level has a visibly greater indentation rather than aligning with its parent
- **AND** the same full depth remains available through semantic levels and keyboard parent/child navigation

#### Scenario: Scrolling the tree leaves controls fixed
- **WHEN** a deep tree requires horizontal or vertical scrolling
- **THEN** the file rows scroll inside the tree viewport without moving search, filters, matching count, Next or exceptional notices
- **AND** filenames retain a usable reading width, file rows stay single-line and keyboard navigation brings the focused filename into view

#### Scenario: Review states remain visually distinct
- **WHEN** file rows represent unviewed, viewed, partly viewed, changed-since-review, skipped and unknown progress
- **THEN** each state has a distinct icon shape and complete accessible wording without a separate visible text badge
- **AND** stale or missing per-file progress uses the unknown icon, not the unviewed or skipped icon

#### Scenario: Source counts are unavailable
- **WHEN** a file has an unavailable addition or removal count
- **THEN** that unavailable count remains absent while any known count retains its exact signed value
- **AND** the file and its review metadata still occupy one line

#### Scenario: Complete metadata survives truncation
- **WHEN** two files with the same basename in different folders have truncated navigator labels and one has three discussion threads
- **THEN** each row's tooltip and accessible name identify its complete safely displayed path, change status, available counts and review state
- **AND** the file with discussions has a passive discussion icon and an exact three-thread description without an extra row or tab stop

#### Scenario: Compact controls retain their meaning
- **WHEN** the PR file navigator is shown with normal pointer input and no exceptional notices
- **THEN** search occupies one controls row and the filter, exact matching-file count and Next control share the next row
- **AND** search and filter retain programmatic labels and Next retains the accessible name Next unreviewed

#### Scenario: Exceptional notices remain visible
- **WHEN** filtering has no matches, the current file falls outside a filter or progress is unavailable
- **THEN** the relevant explanation and applicable clear-filter action remain readable without being hidden in file icons or truncated into the compact toolbar
- **AND** extra notice height does not cause individual file entries to gain metadata lines

#### Scenario: Keyboard activation does not change review state
- **WHEN** a reviewer enters the tree, moves through folders with arrow keys and activates a file with Enter or Space
- **THEN** the existing roving focus and semantic directory levels are preserved and the file section is revealed
- **AND** review/discussion icons add no focus targets and activation makes no viewed-mark, inclusion or provider-review write

#### Scenario: Compact rows follow responsive presentation
- **WHEN** a main or detached PR view is narrowed until the navigator is available through its disclosure
- **THEN** search, filters and jump controls remain reachable and file entries retain the same one-line metadata layout
- **AND** existing text size, theme legibility, text scaling, coarse-pointer targets and diff-layout fallback remain intact

#### Scenario: Non-PR diff presentation is unchanged
- **WHEN** commit detail renders the shared diff view without optional PR navigation metadata
- **THEN** its existing navigator rows, folder behavior, counts and layout remain unchanged
- **AND** no PR controls, review icons or discussion indicators are introduced

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

Existing provider panels SHALL retain their provider scopes, sections, counts, collapse controls, open/worktree gestures, PR numbers and canonical active-PR highlighting. The GitHub and BitBucket panels in the desktop application and browser skin SHALL NOT provide a free-text search field or narrow rows by a text query. They SHALL offer one local filter: All, Failing checks, Conflicts or Drafts, plus Review requested for GitHub only. All SHALL show every row in the provider snapshot.

Header totals SHALL remain snapshot totals; active filtering SHALL show matching/total counts and a clear-filter affordance that restores All. No-match results SHALL be distinct from an empty provider list or authentication failure. Filters SHALL NOT trigger provider reads. Initial row order SHALL follow the snapshot; subsequent snapshots SHALL preserve surviving row order within each section, append newcomers in snapshot order and remove missing rows without remounting a focused surviving row. The removal of provider-panel search SHALL NOT change the Files navigator's path search.

#### Scenario: Provider widgets have no search bar
- **WHEN** an expanded GitHub or BitBucket panel lists pull requests in the desktop application or browser skin
- **THEN** its controls contain the existing labelled filter selector without a search field or an empty search row
- **AND** All shows every snapshot row regardless of title, repository, number or branch names

#### Scenario: Finding a requested review
- **WHEN** the reader selects GitHub Review requested
- **THEN** only requested-review rows are listed with matching/total counts while header totals remain snapshot totals
- **AND** no extra provider request occurs

#### Scenario: Clearing a status filter
- **WHEN** the reader activates Clear while a non-default panel filter is selected
- **THEN** All is selected and every snapshot row is shown again
- **AND** matching/total counts and Clear disappear without a provider request

#### Scenario: A filter matches no rows
- **WHEN** a provider snapshot has rows but none match the selected filter
- **THEN** the panel shows a no-match message, zero matching rows, the unchanged snapshot totals and Clear
- **AND** it does not show an empty-provider or authentication-failure message

#### Scenario: A refresh changes urgency
- **WHEN** a surviving row changes from passing to failing checks during a panel refresh
- **THEN** its status updates without moving relative to other surviving rows in its section
- **AND** a focused row retains focus

#### Scenario: Canonical active selection
- **WHEN** the current PR address uses different casing from its panel row
- **THEN** that row remains visibly and accessibly marked as the current PR

#### Scenario: File search is unchanged
- **WHEN** the reader searches a path in the PR Files navigator
- **THEN** the existing case-insensitive path search and review filters remain available
- **AND** provider-panel filtering remains independent of file search

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
