## MODIFIED Requirements

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
