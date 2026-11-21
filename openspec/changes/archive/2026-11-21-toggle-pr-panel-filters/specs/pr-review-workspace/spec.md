## MODIFIED Requirements

### Requirement: Provider-Panel Triage

The GitHub and BitBucket panels in the desktop application and browser skin SHALL retain their provider scopes, sections, snapshot header totals, collapse controls, open/worktree gestures, PR numbers and canonical active-PR highlighting.

#### Scenario: Provider widgets have no search bar
- **WHEN** an expanded GitHub or BitBucket panel initially lists pull requests in the desktop application or browser skin
- **THEN** every snapshot row is shown without a filter selector, search field or empty controls row
- **AND** a labelled filter toggle is available beside the heading with its expanded state reported as false

#### Scenario: Revealing the status filter
- **WHEN** the reader activates a panel's filter toggle while its controls are hidden
- **THEN** that panel shows the labelled filter selector with All selected, without a search field
- **AND** every snapshot row remains visible regardless of title, repository, number or branch names and the panel remains expanded

#### Scenario: Hiding an active filter
- **WHEN** the reader hides a panel's filter controls while a non-default filter is selected
- **THEN** every snapshot row is shown again and the controls row, matching counts, Clear and any no-match message disappear
- **AND** revealing the controls again starts at All without a provider request

#### Scenario: Provider toggles are independent
- **WHEN** the reader reveals or hides the GitHub panel's filter controls while both providers' panels are present
- **THEN** the BitBucket panel's filter visibility and selected filter are unchanged
- **AND** toggling BitBucket's controls likewise leaves GitHub's controls unchanged

#### Scenario: Remounting starts with filters off
- **WHEN** a panel with visible filter controls is unmounted and mounted again, including after a reload
- **THEN** its filter controls start hidden and every snapshot row is shown
- **AND** no application setting or other viewer surface is changed

#### Scenario: Filter toggle remains accessible
- **WHEN** the reader reaches the filter toggle by keyboard or uses it with a coarse pointer
- **THEN** the provider-specific label, expanded state and visible focus treatment identify the control, and Enter or Space reveals or hides the controls without collapsing the panel
- **AND** the coarse-pointer target is at least 44 by 44 pixels and the toggle remains reachable in a narrow panel

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

## ADDED Requirements

### Requirement: Provider-Panel Status Filters

The GitHub and BitBucket panels SHALL offer one local status filter: All, Failing checks, Conflicts or Drafts, plus Review requested for GitHub only. All SHALL show every snapshot row. Panels SHALL NOT offer free-text search or narrow rows by a text query. Filtering and toggling controls SHALL NOT trigger provider reads or change the Files navigator's path search or review filters.

#### Scenario: Status options follow the provider
- **WHEN** the reader reveals the status selector in a GitHub or BitBucket panel
- **THEN** All, Failing checks, Conflicts and Drafts are available, with Review requested available only for GitHub
- **AND** selecting a status uses the existing snapshot without a provider request

#### Scenario: Text never narrows panel rows
- **WHEN** All is effective in a provider panel
- **THEN** every snapshot row is shown regardless of its title, repository, PR number or branch names
- **AND** the panel has no text-search field or empty search row

#### Scenario: Panel filters leave file search independent
- **WHEN** the reader changes a panel's status filter or toggles its controls
- **THEN** the PR Files navigator retains its existing case-insensitive path search and review filters
- **AND** the panel action triggers no provider read

### Requirement: Optional Provider-Panel Filter Controls

Provider panels SHALL initially hide filter controls without an empty row. The heading's filter toggle SHALL reveal them with All selected. Hiding SHALL restore All and remove controls, matching counts, Clear and any no-match message. Filter selection and visibility SHALL be local to each mounted panel, independent of other providers and viewer surfaces, never application settings or persisted preferences. Remounting SHALL start hidden.

#### Scenario: Mounted panels begin unfiltered
- **WHEN** an expanded provider panel is first mounted with a nonempty snapshot
- **THEN** every row is shown and no filter selector or empty controls row is rendered
- **AND** the filter toggle reports its controls as collapsed

#### Scenario: Revealing and hiding reset to All
- **WHEN** the reader hides controls with a non-default filter selected, then reveals them again
- **THEN** all snapshot rows are restored on hiding and the selector reopens at All
- **AND** matching counts, Clear and any no-match message disappear on hiding, without leaving an active hidden filter

#### Scenario: Filter state remains local
- **WHEN** the reader changes a provider panel's selected filter or control visibility in one viewer surface
- **THEN** the other provider's selection and control visibility and every other viewer surface remain unchanged
- **AND** no application setting or persisted preference is changed

#### Scenario: A remounted panel starts hidden
- **WHEN** a panel with visible controls is unmounted and mounted again, including after a reload
- **THEN** its controls start hidden and All is effective

### Requirement: Accessible Provider-Panel Filter Toggle

The heading's filter toggle SHALL be separate from collapse and have a provider-specific accessible label, expanded state and visible keyboard-focus treatment. While its controls are visible, it SHALL identify them programmatically. Enter and Space SHALL reveal or hide controls without collapsing the panel. The toggle SHALL remain reachable in a narrow panel and provide a target of at least 44 by 44 pixels for coarse pointers.

#### Scenario: Keyboard disclosure has an accessible relationship
- **WHEN** the reader focuses the filter toggle and presses Enter or Space
- **THEN** the focus treatment is visible, the provider-specific name and expanded state identify the control, and its visibility action leaves the panel expanded
- **AND** while the controls are visible the toggle identifies their container programmatically

#### Scenario: Narrow and touch panels retain the control
- **WHEN** the panel is narrowed or used with a coarse pointer
- **THEN** its heading can shrink without clipping the filter toggle
- **AND** the coarse-pointer toggle target is at least 44 by 44 pixels and still reveals the selector

### Requirement: Provider-Panel Filter Feedback

Active status filtering SHALL show matching/total counts and a Clear control that restores All. Header totals SHALL remain snapshot totals. Filtered no-match results SHALL be distinguishable from empty provider snapshots and authentication failures. Selecting All SHALL remove matching counts and Clear.

#### Scenario: Filter counts do not replace snapshot totals
- **WHEN** a selected status matches two of eight snapshot rows
- **THEN** the panel shows two matching rows and a two-of-eight matching count
- **AND** the header retains its original snapshot totals

#### Scenario: Empty matches are not an empty provider
- **WHEN** a nonempty provider snapshot has no rows matching the selected status
- **THEN** the panel shows a no-match message, zero matching rows, matching/total counts and Clear
- **AND** its snapshot header totals remain unchanged and no empty-provider or authentication-failure message is shown

#### Scenario: Clear restores the complete list
- **WHEN** the reader activates Clear while a non-default status filter is selected
- **THEN** All is selected and every snapshot row is shown
- **AND** matching counts and Clear disappear while the selector remains visible

### Requirement: Stable Provider-Panel Row Order

Initial row order SHALL follow the provider snapshot. On subsequent snapshots, each section SHALL preserve surviving row order, append newcomers in snapshot order and remove missing rows without remounting a focused surviving row.

#### Scenario: Initial rows follow their section's snapshot
- **WHEN** a provider panel first receives a snapshot
- **THEN** each section lists its rows in snapshot order rather than reordering them by urgency

#### Scenario: Snapshot updates preserve surviving rows
- **WHEN** a later snapshot removes a row, adds newcomers and changes a surviving row from passing to failing checks
- **THEN** the missing row is removed, surviving rows keep their relative section order and newcomers are appended in snapshot order
- **AND** the changed check signal updates without moving or remounting a focused surviving row
