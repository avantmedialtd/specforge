# archive-browser Specification

## Purpose

Defines the Archive view: a read-only, single-workspace browser over a registered workspace's `openspec/changes/archive/` tree, rendered in the detail pane and reached from the sidebar footer's Archive entrypoint. It covers the workspace dropdown that scopes the view, the newest-first listing whose identifier and `YYYY-MM-DD` date come from the `<YYYY-MM-DD>-<id>` directory name, the in-memory search filter, per-change artifact navigation backed by the `list_archived` and `archived_artifact_status` commands, live refresh on archive-transition events only while the view is open, and the confinement of every archive read to registered workspaces. Its defining constraint is that archived content is loaded on demand and never parsed on the watcher's aggregation path — the active-change tree, the footer entrypoint itself, and the shared markdown rendering this view reuses belong to `spec-browser`.

## Requirements
### Requirement: Archive View

The application SHALL provide an Archive view rendered in the detail (center) pane, reached from the Archive entrypoint in the sidebar footer (see *Archive Entrypoint in Sidebar Footer* in the `spec-browser` capability). The Archive view SHALL present, for a single selected workspace, the list of that workspace's archived changes — the changes under its `openspec/changes/archive/` directory.

The view SHALL contain, from top to bottom: a workspace selector (see *Workspace Scoping via Dropdown*), a search field (see *Search Within the Selected Workspace's Archive*), and the result list. Each list row SHALL display the change's archive date and its identifier (or its proposal title when one is available). The view SHALL be read-only: it SHALL NOT modify, move, or un-archive any change.

#### Scenario: Archive view lists the selected workspace's archived changes

- **WHEN** the user opens the Archive view for a workspace that has archived changes
- **THEN** the view renders one row per archived change in that workspace's `openspec/changes/archive/` directory
- **AND** each row shows the change's archive date and its identifier or proposal title

#### Scenario: Archive view for a workspace with no archived changes

- **WHEN** the user opens the Archive view for a workspace whose `openspec/changes/archive/` directory is empty or absent
- **THEN** the view renders an empty-state message in place of the list
- **AND** no error is shown

### Requirement: Workspace Scoping via Dropdown

The Archive view SHALL be scoped to exactly one workspace at a time, selected from a dropdown control listing the registered workspaces by their display name (falling back to the folder basename). Changing the selection SHALL load and render that workspace's archive (see *On-Demand, Off-Hot-Path Loading*). The view SHALL NOT pool archived changes across multiple workspaces.

When exactly one workspace is registered, the view SHALL show that workspace's archive directly; the dropdown MAY be rendered in a disabled or non-interactive form, since there is no alternative to choose.

#### Scenario: Switching workspace re-scopes the list

- **WHEN** the Archive view is showing workspace A's archive
- **AND** the user selects workspace B from the dropdown
- **THEN** the view replaces the list with workspace B's archived changes
- **AND** workspace A's rows are no longer shown

#### Scenario: Single registered workspace needs no selection

- **WHEN** only one workspace is registered
- **AND** the user opens the Archive view
- **THEN** that workspace's archive is shown without requiring a selection
- **AND** the dropdown presents no alternative workspace to choose

### Requirement: On-Demand, Off-Hot-Path Loading

Archived-change content SHALL be loaded only in response to the user opening the Archive view or changing the selected workspace, and SHALL NOT be parsed as part of the watcher's per-batch workspace aggregation. The aggregation that maintains the active tree and badge SHALL NOT parse any archived change — it SHALL NOT read any archived change's `proposal.md`, `tasks.md`, or capability spec files. It MAY enumerate the `openspec/changes/archive/` directory (a directory listing only) to distinguish an archived change from a deleted one for its event diff.

Loading SHALL be tiered to what is displayed:

- The list SHALL be populated from a **lightweight listing** of the selected workspace's archive — each entry comprising the change identifier, the archive date, and the proposal title. The identifier and date SHALL be derived from the archive directory name (`<YYYY-MM-DD>-<id>`) without reading the change's files; the title SHALL be read from the change's `proposal.md` heading only.
- The full content of an archived change SHALL be read only when that change is selected for reading (see *Read-Only Artifact Navigation*), via the existing artifact-read path.

#### Scenario: Watcher batch does not parse the archive

- **WHEN** a file under a registered workspace's active `openspec/changes/` tree is modified, triggering a watcher re-aggregation
- **THEN** no archived change is parsed as a side effect of the re-aggregation — no archived `proposal.md`, `tasks.md`, or capability spec file is read
- **AND** any access to `openspec/changes/archive/` is limited to a directory listing used to tell archived changes from deleted ones

#### Scenario: Opening the view loads the listing on demand

- **WHEN** the user opens the Archive view for a workspace
- **THEN** the application reads that workspace's archive listing at that point
- **AND** each listing entry's identifier and date are taken from the archive directory name without reading the change's task or spec files

#### Scenario: Archived change content is read only on selection

- **WHEN** the Archive view's list is rendered
- **AND** the user has not selected any individual archived change
- **THEN** no archived change's `tasks.md` or capability spec files have been read

### Requirement: Newest-First Ordering and Date Labels

The Archive view's list SHALL be ordered reverse-chronologically by archive date, most recently archived first. Each row SHALL display the archive date in `YYYY-MM-DD` form, sourced from the archive directory-name prefix.

#### Scenario: List is ordered newest-first

- **WHEN** the Archive view renders a workspace's archived changes
- **THEN** the change with the most recent archive date appears first
- **AND** rows descend by archive date

#### Scenario: Row shows the archive date

- **WHEN** an archived change's directory is named `<YYYY-MM-DD>-<id>`
- **THEN** its row displays the `YYYY-MM-DD` date
- **AND** the date is not re-derived from file contents or modification times

### Requirement: Search Within the Selected Workspace's Archive

The Archive view SHALL provide a search field that filters the currently displayed list by a case-insensitive substring match against each archived change's identifier and proposal title. The filter SHALL apply only to the selected workspace's already-loaded listing and SHALL NOT trigger additional filesystem reads. Clearing the search field SHALL restore the full list for the selected workspace.

#### Scenario: Search narrows the list

- **WHEN** the Archive view is showing a workspace's archived changes
- **AND** the user types a substring into the search field
- **THEN** the list shows only the changes whose identifier or proposal title contains that substring, case-insensitively
- **AND** no additional archive files are read to perform the filtering

#### Scenario: Clearing search restores the full list

- **WHEN** a search filter is active in the Archive view
- **AND** the user clears the search field
- **THEN** the full list for the selected workspace is shown again

### Requirement: Read-Only Artifact Navigation

Selecting an archived change in the Archive view SHALL render that change's artifacts (proposal, design, tasks, capability specs) in read-only form, using the same markdown-rendering path as active-change artifacts, reading from the change's `openspec/changes/archive/<YYYY-MM-DD>-<id>/` directory. Selecting an archived change SHALL NOT modify the change on disk.

The reader SHALL present a control for switching between the change's artifacts. The control SHALL offer only the artifacts that exist on disk for that change — determined on demand when the change is opened — with one entry per capability spec. The proposal SHALL be shown first by default. Determining which artifacts exist is per-change and on demand, and SHALL NOT occur on the watcher's aggregation path.

The reader SHALL additionally display the archived change's **on-disk directory name** — the dated `<YYYY-MM-DD>-<id>` folder under `openspec/changes/archive/` — alongside the title it already shows. The `archive/` path prefix that the reader uses to address the artifact SHALL NOT appear in the displayed name: what is shown is the folder's own name, so it can be used directly as a filesystem identifier. The dated directory name is displayed in preference to the undated change id because the directory is what exists on disk.

That directory name SHALL **copy itself when clicked**, exactly as specified by the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability, which the reader shares: a single primary click SHALL place exactly that name on the clipboard and select it as confirmation, the outcome SHALL be indicated and announced, a refused write SHALL leave the value selected, and the value SHALL be keyboard-activatable as a tab stop without introducing any global chord. (This supersedes the previous contract, which specified selection only and forbade an application clipboard write.) No branch chip SHALL be rendered: an archived change has no live worktree, and the worktree its artifact is read from routinely hosts other, active changes whose branch was never the archived change's.

The reader's identity is not flush with the top of the window — it sits below the archive header and the artifact-switch control — so it takes no titlebar-strip clearance.

#### Scenario: Opening an archived change renders its proposal

- **WHEN** the user selects an archived change in the Archive view
- **THEN** the application renders that change's proposal markdown read from its archive directory
- **AND** the underlying files are not modified

#### Scenario: Switching to another artifact of the open change

- **WHEN** an archived change is open and the user activates the artifact-switch control for its design, tasks, or a capability spec
- **THEN** the reader renders that artifact's markdown from the same archive directory
- **AND** only artifacts that exist on disk for that change are offered as switch targets

#### Scenario: Reader names the archive directory

- **WHEN** an archived change is open in the reader
- **THEN** the reader displays the change's dated archive directory name in full
- **AND** the displayed name carries no `archive/` prefix
- **AND** the reader continues to show the change's title as it does today

#### Scenario: One click copies the whole directory name

- **WHEN** the user clicks once on the archive directory name
- **THEN** exactly that directory name is placed on the clipboard
- **AND** it is also selected, as confirmation of what was copied
- **AND** the outcome is indicated and announced

#### Scenario: An archived change shows no branch

- **WHEN** an archived change is open in the reader
- **AND** the worktree its artifacts are read from is on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the reader

#### Scenario: The archive reader needs no titlebar clearance

- **WHEN** the application runs in the native window on macOS and an archived change is open
- **THEN** the directory name is already clear of the titlebar drag region, below the archive header
- **AND** it is clickable without any additional offset

### Requirement: Live Refresh of the Open Archive View

While the Archive view is open and showing a given workspace, the application SHALL refresh that workspace's listing in response to an archive-transition event for that workspace (a change moving into or out of `openspec/changes/archive/`), so a change archived while the view is open appears without the user reopening the view. While the Archive view is closed, no such refresh work SHALL be performed.

#### Scenario: Change archived while the view is open appears

- **WHEN** the Archive view is open and showing workspace A
- **AND** a change in workspace A is archived (its last active instance moves into `openspec/changes/archive/`)
- **THEN** the new archived change appears in the Archive view's list within the watcher debounce window
- **AND** the user does not have to reopen the view

#### Scenario: No refresh work while the view is closed

- **WHEN** the Archive view is not open
- **AND** a change is archived in some workspace
- **THEN** the application performs no archive-listing read in response to that event

### Requirement: Archive Reads Are Confined to Registered Workspaces

Listing a workspace's archived changes and reporting an archived change's artifact status SHALL be authorized only when the workspace is a registered (or registry-discovered) workspace, and a caller-supplied workspace that is not in the registry SHALL be refused rather than read. The workspace SHALL be matched by its canonical path against the registry's known workspace folders using the same canonicalization the registry keys on, and the check SHALL be enforced at the shared application boundary so it applies to every frontend and transport, matching the artifact-read confinement in the `spec-browser` capability. The existing sanitization of the archive directory name (rejecting path separators and `..`) SHALL remain in force.

#### Scenario: Archive listing for an unregistered workspace is refused

- **WHEN** the archive listing or archived-artifact-status operation is invoked for a workspace path that is not a registered or registry-discovered workspace
- **THEN** the operation is refused with an error
- **AND** no directory under that path is enumerated and no archived file is read

#### Scenario: Archive listing for a registered workspace succeeds

- **WHEN** the archive listing or archived-artifact-status operation is invoked for a registered workspace
- **THEN** it returns that workspace's archived changes or the archived change's artifact status as before

#### Scenario: Archive directory-name sanitization still applies

- **WHEN** an archived-artifact-status request supplies a directory name containing a path separator or a `..` segment
- **THEN** it is rejected as an invalid archive directory name, independently of the registration check

