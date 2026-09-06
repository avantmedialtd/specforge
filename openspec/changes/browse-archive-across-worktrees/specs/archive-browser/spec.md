## ADDED Requirements

### Requirement: Union Archive Listing Across a Repository's Worktrees

The Archive view SHALL be scoped to one **top-level row** at a time — a repository group or a flat workspace — selected from a scope selector listing those rows by their display name (falling back to the folder basename). For a repository, the view SHALL present the union of the archived changes found across **every tracked worktree of that repository**, including worktrees SpecForge auto-discovered rather than ones the user registered directly. For a flat workspace, the union is over that single folder and the view behaves exactly as it did before.

This supersedes the previous contract, under which the view was scoped to a single registered workspace and was forbidden from pooling archived changes across workspaces. That prohibition made a change archived inside a feature worktree unreachable until its branch merged, because the archiving worktree was auto-discovered and so appeared in no selectable list. Pooling is now required, and the previous behaviour survives as the degenerate case of a repository with one tracked worktree.

The union SHALL be de-duplicated on the change's **bare logical identifier** — the archive directory name with at most one leading `YYYY-MM-DD-` prefix removed — so that one logical change is one row. De-duplication SHALL NOT key on the raw archive directory name: the date prefix records the day the change was archived *in that worktree*, so two worktrees that archived the same change on different days would otherwise render as two rows, which is the duplication a union exists to remove.

Each row SHALL retain every copy it collapsed, each copy identified by the pair of the worktree that holds it and the archive directory name within that worktree. That pair, not the logical identifier, is what addresses a read.

When exactly one top-level row is registered, the view SHALL show that row's archive directly; the scope selector MAY be rendered in a disabled or non-interactive form, since there is no alternative to choose.

#### Scenario: A change archived in only one worktree is listed

- **WHEN** a change is archived inside a repository's feature worktree and its branch has not merged
- **AND** the user opens the Archive view scoped to that repository
- **THEN** the change appears in the listing
- **AND** it appears whether the archiving worktree was registered by the user or auto-discovered by SpecForge

#### Scenario: The same change in several worktrees is one row

- **WHEN** a repository has three tracked worktrees and all three contain the archived change `add-thing`
- **THEN** the listing contains exactly one row for `add-thing`
- **AND** that row records all three copies

#### Scenario: Differing date prefixes collapse to one row

- **WHEN** one worktree holds `openspec/changes/archive/2026-06-04-add-thing/` and another holds `openspec/changes/archive/2026-06-05-add-thing/`
- **THEN** the listing contains exactly one row for `add-thing`
- **AND** that row records both copies with their respective directory names

#### Scenario: A legacy un-dated directory collapses with its dated twin

- **WHEN** one worktree holds `openspec/changes/archive/add-thing/` and another holds `openspec/changes/archive/2026-06-04-add-thing/`
- **THEN** both are recognised as the same logical change and yield one row

#### Scenario: Switching scope re-scopes the list

- **WHEN** the Archive view is showing repository A's archive
- **AND** the user selects repository B from the scope selector
- **THEN** the view replaces the list with repository B's union listing
- **AND** repository A's rows are no longer shown

#### Scenario: A flat workspace is unaffected

- **WHEN** the Archive view is scoped to a flat (non-git) workspace
- **THEN** the listing contains that folder's archived changes and no others

### Requirement: Copy Selection Within an Opened Archived Change

When an archived change that exists in more than one tracked worktree is opened for reading, the reader SHALL present a control for choosing which worktree's copy is rendered, and choosing a copy SHALL re-point **only that reader**. Selecting a copy SHALL NOT change the listing's scope, SHALL NOT close the change being read, and SHALL NOT clear the search filter.

When the change exists in exactly one tracked worktree, the control SHALL be rendered as a plain, non-interactive label naming that worktree rather than as a chooser, since there is no alternative to select.

Copies SHALL be labelled by the workspace's display name, falling back to the worktree folder's basename. A copy SHALL NOT be labelled by the branch its worktree is on: see the *Read-Only Artifact Navigation* requirement, which forbids naming the read-from worktree's branch anywhere in the reader because that worktree routinely hosts other, active changes whose branch was never the archived change's.

The control SHALL NOT assert that copies are identical. Archived content is read from the working tree rather than from git history, so two copies can differ without any commit — through an uncommitted edit, through a worktree checked out at a commit that predates a correction to the archived record, or in a workspace that does not track its archive at all. Where the application can cheaply tell that copies differ, it SHALL indicate that rather than presenting them as interchangeable.

#### Scenario: Switching to another worktree's copy

- **WHEN** an archived change present in two worktrees is open in the reader
- **AND** the user selects the second worktree's copy
- **THEN** the reader renders that worktree's copy of the currently shown artifact
- **AND** the change remains open and the listing is not refetched

#### Scenario: A single-copy change shows a label, not a chooser

- **WHEN** an archived change present in exactly one tracked worktree is open in the reader
- **THEN** the worktree is named as a plain label
- **AND** no copy chooser is offered

#### Scenario: Copies are named by workspace, never by branch

- **WHEN** the copy control lists the worktrees holding an archived change
- **AND** those worktrees are on named branches
- **THEN** each copy is labelled by its workspace display name or worktree folder basename
- **AND** no branch name is shown

#### Scenario: Divergent copies are not presented as identical

- **WHEN** two worktrees hold the same archived change with different content on disk
- **THEN** the copy control does not present them as interchangeable
- **AND** both copies remain openable

## MODIFIED Requirements

### Requirement: Archive View

The application SHALL provide an Archive view rendered in the detail (center) pane, reached from the Archive entrypoint in the sidebar footer (see *Archive Entrypoint in Sidebar Footer* in the `spec-browser` capability). The Archive view SHALL present, for a single selected top-level row — a repository group or a flat workspace — the list of that row's archived changes, pooled across its tracked worktrees (see *Union Archive Listing Across a Repository's Worktrees*).

The view SHALL contain, from top to bottom: a scope selector, a search field (see *Search Within the Selected Workspace's Archive*), and the result list. Each list row SHALL display the change's archive date and its identifier (or its proposal title when one is available). The view SHALL be read-only: it SHALL NOT modify, move, or un-archive any change.

#### Scenario: Archive view lists the selected row's archived changes

- **WHEN** the user opens the Archive view for a top-level row that has archived changes
- **THEN** the view renders one row per archived logical change found across that row's tracked worktrees
- **AND** each row shows the change's archive date and its identifier or proposal title

#### Scenario: Archive view for a row with no archived changes

- **WHEN** the user opens the Archive view for a top-level row whose tracked worktrees have an empty or absent `openspec/changes/archive/` directory
- **THEN** the view renders an empty-state message in place of the list
- **AND** no error is shown

### Requirement: On-Demand, Off-Hot-Path Loading

Archived-change content SHALL be loaded only in response to the user opening the Archive view or changing the selected scope, and SHALL NOT be parsed as part of the watcher's per-batch workspace aggregation. The aggregation that maintains the active tree and badge SHALL NOT parse any archived change — it SHALL NOT read any archived change's `proposal.md`, `tasks.md`, or capability spec files. It MAY enumerate the `openspec/changes/archive/` directory (a directory listing only) to distinguish an archived change from a deleted one for its event diff.

Loading SHALL be tiered to what is displayed:

- The list SHALL be populated from a **lightweight listing** of each tracked worktree in the selected scope — each entry comprising the change identifier, the archive date, and the proposal title. The identifier and date SHALL be derived from the archive directory name (`<YYYY-MM-DD>-<id>`) without reading the change's files; the title SHALL be read from the change's `proposal.md` heading only.
- The full content of an archived change SHALL be read only when that change is selected for reading (see *Read-Only Artifact Navigation*), and only for the copy being rendered (see *Copy Selection Within an Opened Archived Change*) — never for every copy the row collapsed.

Extending the listing from one worktree to a repository's tracked worktrees SHALL NOT move any of this work onto the aggregation path: the per-worktree unit of work is unchanged, only the number of worktrees it is performed for, and it is performed on demand.

#### Scenario: Watcher batch does not parse the archive

- **WHEN** a file under a registered workspace's active `openspec/changes/` tree is modified, triggering a watcher re-aggregation
- **THEN** no archived change is parsed as a side effect of the re-aggregation — no archived `proposal.md`, `tasks.md`, or capability spec file is read
- **AND** any access to `openspec/changes/archive/` is limited to a directory listing used to tell archived changes from deleted ones

#### Scenario: Opening the view loads the listing on demand

- **WHEN** the user opens the Archive view for a top-level row
- **THEN** the application reads each of that row's tracked worktrees' archive listings at that point
- **AND** each listing entry's identifier and date are taken from the archive directory name without reading the change's task or spec files

#### Scenario: Archived change content is read only on selection

- **WHEN** the Archive view's list is rendered
- **AND** the user has not selected any individual archived change
- **THEN** no archived change's `tasks.md` or capability spec files have been read

#### Scenario: Only the rendered copy is read

- **WHEN** the user opens an archived change that exists in three tracked worktrees
- **THEN** only the copy being rendered has its artifact content read
- **AND** the other two copies' artifact files are not read until one of them is selected

### Requirement: Newest-First Ordering and Date Labels

The Archive view's list SHALL be ordered reverse-chronologically by archive date, most recently archived first. Each row SHALL display the archive date in `YYYY-MM-DD` form, sourced from the archive directory-name prefix.

Where a row collapses copies carrying different date prefixes, the row SHALL be dated and ordered by the **newest** such date, so a change re-archived later in a second worktree sorts by its most recent archival rather than its earliest. A row whose copies are all un-dated legacy directories SHALL display no date, as before.

Ordering SHALL be total and deterministic: rows sharing a date SHALL fall back to a stable tie-break so the listing does not reorder between two reads of unchanged content.

#### Scenario: List is ordered newest-first

- **WHEN** the Archive view renders a scope's archived changes
- **THEN** the change with the most recent archive date appears first
- **AND** rows descend by archive date

#### Scenario: Row shows the archive date

- **WHEN** an archived change's directory is named `<YYYY-MM-DD>-<id>`
- **THEN** its row displays the `YYYY-MM-DD` date
- **AND** the date is not re-derived from file contents or modification times

#### Scenario: A row collapsing two dates shows the newer one

- **WHEN** a row collapses a copy dated `2026-06-04` and a copy dated `2026-06-05`
- **THEN** the row displays `2026-06-05`
- **AND** the row is ordered as if archived on `2026-06-05`

#### Scenario: Rows sharing a date keep a stable order

- **WHEN** two rows carry the same archive date
- **THEN** their relative order is the same on every read of unchanged content

### Requirement: Live Refresh of the Open Archive View

While the Archive view is open and showing a given scope, the application SHALL refresh that scope's listing in response to an archive-transition event for **any tracked worktree within it** (a change moving into or out of `openspec/changes/archive/`), so a change archived while the view is open appears without the user reopening the view. This includes an archival that happens in a worktree other than the one whose copy is currently being read. While the Archive view is closed, no such refresh work SHALL be performed.

#### Scenario: Change archived while the view is open appears

- **WHEN** the Archive view is open and showing repository A
- **AND** a change in one of repository A's tracked worktrees is archived
- **THEN** the new archived change appears in the Archive view's list within the watcher debounce window
- **AND** the user does not have to reopen the view

#### Scenario: Archival in a sibling worktree refreshes the listing

- **WHEN** the Archive view is open and showing repository A, with a change open for reading from worktree W1
- **AND** a change is archived in a different tracked worktree W2 of repository A
- **THEN** the listing refreshes to include it
- **AND** the change being read from W1 stays open

#### Scenario: No refresh work while the view is closed

- **WHEN** the Archive view is not open
- **AND** a change is archived in some workspace
- **THEN** the application performs no archive-listing read in response to that event

### Requirement: Archive Reads Are Confined to Registered Workspaces

Listing a workspace's archived changes and reporting an archived change's artifact status SHALL be authorized only when the workspace is a registered (or registry-discovered) workspace, and a caller-supplied workspace that is not in the registry SHALL be refused rather than read. The workspace SHALL be matched by its canonical path against the registry's known workspace folders using the same canonicalization the registry keys on, and the check SHALL be enforced at the shared application boundary so it applies to every frontend and transport, matching the artifact-read confinement in the `spec-browser` capability. The existing sanitization of the archive directory name (rejecting path separators and `..`) SHALL remain in force.

A registry-discovered worktree SHALL be authorized for these reads exactly as a user-registered workspace is. The union listing depends on this, so it SHALL be verified rather than left implicit: narrowing the check to user-registered folders would silently empty the union of precisely the worktrees it exists to reach.

The repository-scoped union listing SHALL be authorized by its repository identifier, accepted only when that identifier matches the canonical git directory of a registered workspace. A repository the user has not registered SHALL be refused rather than enumerated, and no worktree of it SHALL be read.

#### Scenario: Archive listing for an unregistered workspace is refused

- **WHEN** the archive listing or archived-artifact-status operation is invoked for a workspace path that is not a registered or registry-discovered workspace
- **THEN** the operation is refused with an error
- **AND** no directory under that path is enumerated and no archived file is read

#### Scenario: Archive listing for a registered workspace succeeds

- **WHEN** the archive listing or archived-artifact-status operation is invoked for a registered workspace
- **THEN** it returns that workspace's archived changes or the archived change's artifact status as before

#### Scenario: Archive listing for a registry-discovered worktree succeeds

- **WHEN** a repository's sibling worktree was auto-discovered rather than registered by the user
- **AND** the archive listing or archived-artifact-status operation is invoked for that worktree
- **THEN** it returns that worktree's archived changes or the archived change's artifact status
- **AND** it is not refused for being absent from the user-registered workspace listing

#### Scenario: Union listing for an unregistered repository is refused

- **WHEN** the repository-scoped union listing is invoked for a repository identifier that matches no registered workspace's git directory
- **THEN** the operation is refused with an error
- **AND** no worktree of that repository is enumerated and no archived file is read

#### Scenario: Archive directory-name sanitization still applies

- **WHEN** an archived-artifact-status request supplies a directory name containing a path separator or a `..` segment
- **THEN** it is rejected as an invalid archive directory name, independently of the registration check

### Requirement: Read-Only Artifact Navigation

Selecting an archived change in the Archive view SHALL render that change's artifacts (proposal, design, tasks, capability specs) in read-only form, using the same markdown-rendering path as active-change artifacts, reading from the change's `openspec/changes/archive/<YYYY-MM-DD>-<id>/` directory **within the worktree whose copy is currently selected** (see *Copy Selection Within an Opened Archived Change*). Selecting an archived change SHALL NOT modify the change on disk.

The reader SHALL present a control for switching between the change's artifacts. The control SHALL offer only the artifacts that exist on disk for that change — determined on demand when the change is opened — with one entry per capability spec. The proposal SHALL be shown first by default. Determining which artifacts exist is per-change and on demand, and SHALL NOT occur on the watcher's aggregation path. When the selected copy changes, the offered artifacts SHALL be re-determined against the newly selected copy, since two copies of one archived change need not contain the same artifacts.

The reader SHALL additionally display the archived change's **on-disk directory name** — the dated `<YYYY-MM-DD>-<id>` folder under `openspec/changes/archive/` — alongside the title it already shows. The `archive/` path prefix that the reader uses to address the artifact SHALL NOT appear in the displayed name: what is shown is the folder's own name, so it can be used directly as a filesystem identifier. The dated directory name is displayed in preference to the undated change id because the directory is what exists on disk. Where copies carry different directory names, the name displayed SHALL be that of the copy currently selected, so it always names a directory that exists in the worktree being read.

That directory name SHALL **copy itself when clicked**, exactly as specified by the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability, which the reader shares: a single primary click SHALL place exactly that name on the clipboard and select it as confirmation, the outcome SHALL be indicated and announced, a refused write SHALL leave the value selected, and the value SHALL be keyboard-activatable as a tab stop without introducing any global chord. No branch chip SHALL be rendered: an archived change has no live worktree, and the worktree its artifact is read from routinely hosts other, active changes whose branch was never the archived change's. This prohibition binds the copy-selection control too — naming the copies is permitted, naming their branches is not.

The reader's identity is not flush with the top of the window — it sits below the archive header and the artifact-switch control — so it takes no titlebar-strip clearance.

#### Scenario: Opening an archived change renders its proposal

- **WHEN** the user selects an archived change in the Archive view
- **THEN** the application renders that change's proposal markdown read from its archive directory
- **AND** the underlying files are not modified

#### Scenario: Switching to another artifact of the open change

- **WHEN** an archived change is open and the user activates the artifact-switch control for its design, tasks, or a capability spec
- **THEN** the reader renders that artifact's markdown from the same archive directory
- **AND** only artifacts that exist on disk for that change are offered as switch targets

#### Scenario: Switching copy re-determines the offered artifacts

- **WHEN** an archived change is open and the user selects a different worktree's copy
- **AND** that copy contains a different set of artifacts on disk
- **THEN** the artifact-switch control offers the artifacts that exist in the newly selected copy

#### Scenario: Reader names the archive directory

- **WHEN** an archived change is open in the reader
- **THEN** the reader displays the change's dated archive directory name in full
- **AND** the displayed name carries no `archive/` prefix
- **AND** the reader continues to show the change's title as it does today

#### Scenario: Reader names the selected copy's directory

- **WHEN** an archived change is open whose copies carry different dated directory names
- **AND** the user selects a particular copy
- **THEN** the displayed directory name is the one that exists in the selected worktree

#### Scenario: One click copies the whole directory name

- **WHEN** the user clicks once on the archive directory name
- **THEN** exactly that directory name is placed on the clipboard
- **AND** it is also selected, as confirmation of what was copied
- **AND** the outcome is indicated and announced

#### Scenario: An archived change shows no branch

- **WHEN** an archived change is open in the reader
- **AND** the worktree its artifacts are read from is on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the reader, including in the copy-selection control

#### Scenario: The archive reader needs no titlebar clearance

- **WHEN** the application runs in the native window on macOS and an archived change is open
- **THEN** the directory name is already clear of the titlebar drag region, below the archive header
- **AND** it is clickable without any additional offset

## REMOVED Requirements

### Requirement: Workspace Scoping via Dropdown

**Reason**: The requirement scoped the Archive view to exactly one registered workspace and explicitly forbade pooling archived changes across workspaces. That prohibition is what made a change archived inside an auto-discovered feature worktree unreachable — the worktree appeared in no selectable list, so the only archive the user could open was one that did not contain the change. Scoping and pooling are now governed by *Union Archive Listing Across a Repository's Worktrees*, which selects a repository or flat workspace rather than a single worktree and pools deliberately.

**Migration**: No user action and no data migration. The removed behaviour is the degenerate case of the replacement: a repository with one tracked worktree, or a flat workspace, lists exactly what it listed before. The dropdown remains in the same position with the same fallback-to-basename labelling; only the pool it selects from changes, from registered workspaces to top-level rows.
