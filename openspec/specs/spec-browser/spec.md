# spec-browser Specification

## Purpose

Defines the master-detail browser surface of the desktop application that lets users navigate the OpenSpec artifacts of every registered workspace and read their rendered markdown content in a single window.
## Requirements

### Requirement: Master-Detail Layout

The main application window SHALL present a master-detail layout of two primary panes — a tree-navigation pane on the left and a content-rendering (detail) pane in the center — plus an optional commit-graph rail on the far right (see the *Commit-Graph Rail Pane* requirement in the `commit-graph` capability). Resizable dividers separate the panes. The tree pane and the rail are each independently hideable (see the *Side-Pane Visibility Toggles* requirement); the detail pane is always visible.

Dragging a divider SHALL be driven by pointer input, so that a mouse, a touch contact, and a pen all resize the panes through the same clamps (see the *Drag Interactions Accept Pointer Input* requirement in the `touch-input` capability).

The shell SHALL size itself to the viewport that is actually visible to the user, and SHALL NOT size itself to a viewport height that assumes retractable browser chrome has been retracted. Because the shell suppresses document scrolling, any part of the layout that exceeds the visible viewport is permanently unreachable — there is no scroll with which to recover it — so the shell SHALL never exceed the visible viewport. In particular, content anchored to the bottom of the sidebar SHALL remain on screen and operable at every viewport height at which the application is usable, including the sidebar footer entrypoints covered by the *Settings Entrypoint in Sidebar Footer* and *Archive Entrypoint in Sidebar Footer* requirements, and any usage-quota strips rendered beneath them.

The detail (center) pane SHALL render one of four targets: an OpenSpec artifact's markdown, a commit's detail view when a commit is selected in the rail, the **Dashboard** (see the *Dashboard Home Surface* requirement in the `dashboard` capability), or the **Archive view** (see the *Archive View* requirement in the `archive-browser` capability) when the Archive entrypoint is active. The Dashboard SHALL be the default target: it is rendered at startup and whenever no artifact and no commit is selected and the Archive view is not open, in place of any "nothing selected" placeholder. The Archive view and the Settings view are modal pane targets toggled from their sidebar entrypoints; while either is open it takes precedence over the artifact/commit/Dashboard target, and closing it returns the pane to whichever of those was selected most recently. The tree drives the artifact target and the rail drives the commit target.

#### Scenario: Panes visible at startup

- **WHEN** the user opens the main window for the first time
- **THEN** the tree pane and the detail pane are visible side by side
- **AND** the commit-graph rail is visible on the far right
- **AND** the detail pane renders the Dashboard (no artifact or commit having been selected, and the Archive view not open)
- **AND** the dividers between the panes can be dragged to adjust their widths

#### Scenario: Shell fits a browser viewport with persistent chrome

- **WHEN** the served web UI is loaded in a browser whose chrome occupies part of the screen and does not retract
- **THEN** the shell's height matches the viewport the browser actually exposes
- **AND** no part of the layout extends below the bottom edge of that viewport

#### Scenario: Sidebar footer entrypoints stay reachable on a short viewport

- **WHEN** the served web UI is loaded at a viewport height short enough that the sidebar tree must scroll
- **THEN** the Settings entrypoint, the Archive entrypoint, and any usage-quota strips beneath them are fully visible
- **AND** each of them can be activated
- **AND** the sidebar tree above them absorbs the reduced height by scrolling

#### Scenario: Detail pane renders the Dashboard by default

- **WHEN** no artifact and no commit is selected and the Archive view is not open
- **THEN** the detail pane renders the Dashboard
- **AND** no "nothing selected" placeholder is shown

#### Scenario: Detail pane renders artifact markdown by default

- **WHEN** the user selects a renderable artifact node in the tree
- **THEN** the detail pane renders that artifact's markdown

#### Scenario: Detail pane renders commit detail when a commit is selected

- **WHEN** the user selects a commit in the commit-graph rail
- **THEN** the detail pane renders that commit's detail view
- **AND** selecting an artifact node in the tree afterwards returns the detail pane to artifact markdown

#### Scenario: Detail pane renders the Archive view when its entrypoint is active

- **WHEN** the user activates the Archive entrypoint
- **THEN** the detail pane renders the Archive view in place of the artifact/commit/Dashboard target
- **AND** closing the Archive view returns the detail pane to the most recently selected artifact, commit, or the Dashboard

### Requirement: Side-Pane Visibility Toggles

The tree-navigation pane (sidebar) and the commit-graph rail SHALL each be independently hideable and restorable, in both the desktop application and the served web UI. Any combination of hidden/shown SHALL be reachable; with both side panes hidden the detail pane SHALL occupy the full window width. The detail pane itself SHALL NOT be hideable.

Each visibility SHALL be togglable by keyboard: Cmd+B (macOS) / Ctrl+B (Windows, Linux) for the sidebar, and Cmd+Alt+B (macOS) / Ctrl+Alt+B (Windows, Linux) for the rail, with the same bindings active in the served web UI.

Each visible side pane SHALL display a collapse affordance (a chevron control) at its top. While a side pane is hidden, a restore affordance SHALL be displayed in the corresponding top corner of the detail pane (top-left for the sidebar, top-right for the rail), so that restoring a pane never requires a keyboard shortcut, a menu, or an application restart.

On a device that reports no hover capability, these collapse and restore affordances SHALL be rendered visibly at rest rather than being revealed by pointer hover, so that pane visibility stays operable where neither hover nor a hardware keyboard is available (see the *Essential Controls Are Discoverable Without Hover* requirement in the `touch-input` capability).

Each pane's visibility SHALL persist across sessions in frontend view state, consistent with how the rail width persists (see the *Commit-Graph Rail Pane* requirement in the `commit-graph` capability); visibility SHALL NOT be stored in application settings. A hidden pane's width SHALL be preserved: restoring the pane SHALL bring back the width it had when hidden, clamped to the window's current constraints. A hidden pane's divider SHALL NOT be rendered.

Pane visibility is ambient view state: it SHALL NOT be part of the Address, the URL, or navigation history (see the `view-routing` capability), and navigating — including Back/Forward — SHALL NOT change pane visibility.

On macOS in the desktop application, while the sidebar is hidden the detail pane SHALL reserve the top clearance for the window controls (traffic lights) and the titlebar drag strip that the sidebar normally provides, so that detail-pane content is not obscured.

#### Scenario: Sidebar toggles independently

- **WHEN** the user presses Cmd/Ctrl+B or activates the sidebar's collapse chevron
- **THEN** the sidebar and its divider are hidden and the detail pane widens to absorb the space
- **AND** the commit-graph rail's visibility is unchanged
- **AND** a restore affordance appears in the detail pane's top-left corner

#### Scenario: Rail toggles independently

- **WHEN** the user presses Cmd/Ctrl+Alt+B or activates the rail's collapse chevron
- **THEN** the rail and its divider are hidden and the detail pane widens to absorb the space
- **AND** the sidebar's visibility is unchanged
- **AND** a restore affordance appears in the detail pane's top-right corner

#### Scenario: Both panes hidden yields full-width content

- **WHEN** the sidebar and the rail are both hidden
- **THEN** the detail pane occupies the full window width
- **AND** restore affordances for both panes remain visible in the detail pane's top corners
- **AND** both keyboard toggles remain active

#### Scenario: Pane affordances are visible at rest without hover

- **WHEN** the served web UI is loaded on a device that reports no hover capability
- **THEN** the visible side panes' collapse chevrons are visible at rest
- **AND** activating one hides its pane and reveals a restore affordance that is likewise visible at rest
- **AND** the pane can be restored without a keyboard

#### Scenario: Restoring a pane recovers its previous width

- **WHEN** the user hides a side pane and later restores it
- **THEN** the pane returns at the width it had when hidden, clamped to fit the current window

#### Scenario: Visibility persists across sessions

- **WHEN** the user hides the rail and quits the application
- **AND** relaunches it
- **THEN** the rail is still hidden and the sidebar is still visible

#### Scenario: Navigation does not change visibility

- **WHEN** a side pane is hidden
- **AND** the user navigates to any address, including via Back/Forward
- **THEN** the pane remains hidden and the address is unaffected by pane visibility

#### Scenario: Hidden sidebar keeps macOS window controls clear

- **WHEN** the sidebar is hidden in the desktop application on macOS
- **THEN** the detail pane's content starts below the traffic-light / titlebar drag area rather than underneath it

### Requirement: Workspace Tree Hierarchy

The tree pane SHALL display tracked workspaces grouped by repository where applicable, as a tree of exactly **two levels**: top-level rows and change rows. For each git repository with at least one tracked workspace, the tree SHALL render a top-level Repo group node containing one row per active logical change. For each non-git workspace, the tree SHALL render a top-level workspace node containing that workspace's changes directly.

A logical change groups every `ChangeInstance` in one repository that shares the same **logical change identifier**: the change's directory name for an active instance, and that directory name with at most one leading `YYYY-MM-DD-` prefix removed for an archived one. The two forms are grouped together deliberately, so that an active instance and an archived instance of one change are comparable — which is what the *Per-Instance Divergence Label* requirement needs in order to report `[stale]` at all.

Grouping them, however, SHALL NOT put an archived instance in front of a consumer that renders active changes. Within a logical change, instances SHALL be partitioned by whether they are archived in their worktree, and only the non-archived partition counts as rendered. An archived instance is summarised from a directory listing rather than parsed — it carries no title, no task counts and no artifacts, because the archive is deliberately never read on the aggregation path (see *On-Demand, Off-Hot-Path Loading* in the `archive-browser` capability).

Inside a Repo group, each logical change SHALL be rendered as exactly **one change row**, whatever its rendered instance count. A change with one rendered instance renders as a two-line row naming its branch; a change with two or more renders the same single row carrying an instance-count chip in the branch chip's place (see *Two-Line Sole-Change-Row Layout*), and the instance to read is chosen in the change header (see *Instance Switcher in the Change Header*). An archived instance SHALL NOT contribute to that count.

A change row is a **leaf**. It SHALL expose no artifact, capability, section or task rows beneath it and no disclosure affordance. Activating a change row SHALL navigate to the change's **default artifact** of its **default instance**: the first artifact present on disk in the fixed order Proposal, Design, Tasks, then the first capability spec in listing order; and the main worktree's instance when it hosts the change, otherwise the first rendered instance in aggregation order. The change's other artifacts are reached from the change header (see *Artifact Tab Strip in the Change Header*). A change with no artifact present SHALL still be selectable, and the detail pane SHALL show its empty state.

The tree SHALL render **active changes only**. Archived logical changes SHALL NOT appear anywhere in the tree — neither in an Active section nor in a separate Archive section. Archived changes are browsed exclusively in the dedicated Archive view (see the *Archive View* requirement in the `archive-browser` capability).

A top-level row is a **disclosure row**, open by default. A top-level row the user closes SHALL stay closed for the rest of the session and SHALL NOT be persisted: no disclosure state of any kind survives a restart, and the tree performs no settings write on a toggle. A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — and opens the workspace file browser for the row's workspace in the detail pane (see the `workspace-file-browser` capability). No placeholder child row SHALL be rendered beneath an empty top-level row. A top-level row whose only changes are archived SHALL therefore render as a leaf with a `0` active count, the same as a row with no changes at all.

#### Scenario: Git repo with multiple worktrees shown as one Repo group

- **WHEN** a repository has three tracked worktrees, two of which contain a change with the same directory name
- **THEN** the tree shows one top-level Repo group for that repository
- **AND** the two-instance change appears as exactly one change row carrying an instance-count chip
- **AND** any single-instance change appears as one change row naming its branch

#### Scenario: An archived instance is not rendered beside its active twin

- **WHEN** a change is archived in one worktree of a repository and still active in another
- **THEN** the tree renders exactly one row for it, counting the active instance only
- **AND** that row carries no instance-count chip
- **AND** no unlabelled or artifact-less row is rendered for the archived copy

#### Scenario: Non-git workspace shown as a standalone top-level node

- **WHEN** a tracked workspace is not inside a git repository
- **THEN** the workspace is rendered as a top-level node (not a Repo group)
- **AND** the workspace's changes are rendered directly underneath without instance aggregation

#### Scenario: Activating a change row opens its default artifact

- **WHEN** the user activates the row of a change whose `proposal.md` is absent and whose `design.md` is present
- **THEN** the detail pane renders that change's design
- **AND** the change header's tab strip shows Design as the active tab

#### Scenario: Activating a multi-instance change row opens the default instance

- **WHEN** the user activates the row of a change hosted in the repository's main worktree and in one feature worktree
- **THEN** the detail pane renders the main worktree's copy of the default artifact
- **AND** the change header's instance switcher marks the main worktree's instance as selected

#### Scenario: A change row exposes nothing beneath it

- **WHEN** a change row is rendered
- **THEN** it has no disclosure chevron
- **AND** no artifact, capability, section or task row is rendered beneath it

#### Scenario: Archived logical changes are not shown in the tree

- **WHEN** every instance of a logical change is under `openspec/changes/archive/` in its worktree
- **THEN** the logical change is not shown anywhere in the tree
- **AND** it is browsable in the Archive view instead

#### Scenario: Empty top-level row renders as a leaf

- **WHEN** a top-level Repo group node or non-git workspace node has zero active changes
- **THEN** the row renders as a leaf with no disclosure chevron and no toggle affordance
- **AND** the row's count badge displays `0`
- **AND** clicking the row updates the tree's selected-node state and applies the same visual selection treatment a non-empty top-level row receives
- **AND** the detail pane shows the workspace file browser for the row's workspace (see the `workspace-file-browser` capability)
- **AND** no placeholder child row (such as "no active changes") is rendered beneath the row

#### Scenario: Top-level row with only archived changes renders as a leaf

- **WHEN** a top-level row has zero active changes but one or more archived changes
- **THEN** the row renders as a leaf with a `0` active count
- **AND** no archived change is rendered beneath it in the tree

#### Scenario: Empty top-level row becomes non-empty when a change is added

- **WHEN** a top-level row was rendering as a leaf because it had zero active changes
- **AND** the watcher reports a new active change for that workspace
- **THEN** the row re-renders as an open disclosure parent
- **AND** the count badge advances from `0` to the new count

#### Scenario: Top-level disclosure does not survive a restart

- **WHEN** the user closes a top-level row and restarts the application
- **THEN** the row renders open
- **AND** no settings write was performed when it was closed

### Requirement: Top-Level Row Display Name and Swatch

The tree pane SHALL render every top-level row — a flat workspace node or a repository group node — using the row's configured display name when one is set, and using the row's derived default name (the folder basename for a flat workspace, the main worktree's basename for a repository group) when none is set. The tree pane SHALL render an 8px filled circular swatch glyph between the row's chevron and its label, in the colour corresponding to the row's configured palette colour, when one is set. When no palette colour is configured the swatch SHALL be omitted.

The swatch SHALL be applied to the top-level row only. Child rows (logical changes, instances, artifact nodes, sections, tasks, capability spec rows) SHALL NOT render a swatch. The row's background SHALL be the default row background regardless of palette colour. The existing selection treatment (a 2px `--accent` `border-left`) SHALL compose with the swatch without modification: the swatch sits in the row's content area, the selection bar lives in the inline-start border slot, and the two signals do not overlap.

#### Scenario: Top-level row uses configured display name

- **WHEN** a flat workspace has a configured display name
- **THEN** its top-level tree row renders with that display name
- **AND** the configured name is also used wherever the row is referenced (for example, the row's accessible label)

#### Scenario: Top-level row falls back to derived name when no display name is configured

- **WHEN** a flat workspace or a repository group has no configured display name
- **THEN** its top-level tree row renders with the folder basename (or main worktree basename, for a repository group)

#### Scenario: Top-level row shows the configured palette colour as a swatch

- **WHEN** a flat workspace or a repository group has a configured palette colour
- **THEN** its top-level tree row renders an 8px filled circular swatch between the chevron and the label, in the colour corresponding to that palette token
- **AND** the row background is the default row background, unchanged by the palette colour
- **AND** child rows below it render no swatch and the default row background

#### Scenario: Top-level row omits the swatch when no palette colour is configured

- **WHEN** a flat workspace or a repository group has no configured palette colour
- **THEN** its top-level tree row renders no swatch
- **AND** the row background is the default row background, indistinguishable from the same row before the presentation store was introduced

#### Scenario: Selection highlight composes with the swatch

- **WHEN** the user selects a top-level row that has a configured palette colour
- **THEN** the row renders both the 2px `--accent` left border bar and the 8px swatch
- **AND** the two signals do not overlap visually (the bar is in the inline-start border slot; the swatch is in the row's content area)
- **AND** the row background is unchanged by the selected state

#### Scenario: Presentation update re-renders the row without a manual refresh

- **WHEN** the user changes the display name or palette colour of a workspace from the Settings view
- **THEN** the corresponding top-level row in the tree pane updates to reflect the new name and swatch without the user having to close and reopen the window or otherwise force a refresh

### Requirement: Inter-Workspace Divider

Successive top-level rows in the tree pane SHALL be separated by a 1px `var(--border)` horizontal hairline. The hairline SHALL be rendered as a `border-top` on every top-level row except the first, so that the first top-level row carries no top border and every subsequent top-level row carries one. The hairline replaces the section-header affordance previously provided by the full-row background tint.

The hairline SHALL apply only to top-level rows (flat workspace nodes and repository group nodes). Child rows SHALL NOT render a `border-top`. The hairline SHALL compose with the row's other visual signals — the swatch in the content area, the selection bar in the inline-start border slot, and any hover/focus state — without modification: it is a cross-axis 1px line and does not occupy the inline-start border slot.

#### Scenario: Second and subsequent workspaces render a hairline

- **WHEN** the tree pane renders two or more top-level rows
- **THEN** the second and every subsequent top-level row resolves a 1px `var(--border)` `border-top`
- **AND** the first top-level row resolves a `border-top` of `0`

#### Scenario: Child rows render no hairline

- **WHEN** a top-level row is expanded
- **THEN** none of its child rows (changes, instances, artifacts, sections, tasks, capability specs) renders a `border-top`
- **AND** the only horizontal separation between successive child rows is the row's vertical padding

#### Scenario: Hairline composes with selection and swatch on the same row

- **WHEN** the user selects a top-level row that is not the first top-level row and that has a configured palette colour
- **THEN** the row simultaneously renders the 1px `var(--border)` `border-top`, the 2px `--accent` `border-left`, and the 8px swatch in the content area
- **AND** no signal visually displaces or hides any other

### Requirement: Markdown Rendering of Leaf Artifacts

Clicking a leaf artifact node (Proposal, Design, Tasks, or an individual capability spec under the Specs node) SHALL render that artifact's markdown file in the detail pane. Rendering MUST support GitHub-Flavored Markdown including syntax-highlighted fenced code blocks.

#### Scenario: Click proposal renders proposal.md

- **WHEN** the user clicks a change's Proposal node
- **THEN** the detail pane shows the rendered content of `proposal.md` for that change

#### Scenario: Click design renders design.md

- **WHEN** the user clicks a change's Design node
- **THEN** the detail pane shows the rendered content of `design.md` for that change

#### Scenario: Click tasks renders tasks.md

- **WHEN** the user clicks a change's Tasks node
- **THEN** the detail pane shows the rendered content of `tasks.md` for that change

#### Scenario: Click individual capability spec renders that spec.md

- **WHEN** the user clicks a child node under the Specs artifact node
- **THEN** the detail pane shows the rendered content of `specs/<capability>/spec.md` for that change

### Requirement: Reactive Updates from Filesystem

The tree pane and every **document surface** SHALL reflect on-disk changes within the watcher's debounce window without requiring user action. A document surface is any surface that renders one markdown document: the detail pane, the workspace file browser's preview region, and a reader window (see the `reader-window` capability). Every guarantee this requirement makes about a document surface binds all three; where the text below says "the pane", it is naming the behaviour of a document surface, not the detail pane alone.

After the watcher finishes processing a debounced batch of filesystem events, the *first* refresh the frontend performs in response to that batch SHALL observe the post-batch state — the UI MUST NOT lag behind by one event for any on-disk change, including content-only changes inside a change directory that is already tracked (artifact file creation, task checkbox toggles, edits to spec or proposal markdown).

A document surface's refresh SHALL re-read the document it is currently rendering. It SHALL be driven by the change notification alone and MUST NOT be conditioned on the workspace named in that notification's payload, because a notification MAY carry any tracked workspace as a carrier rather than the workspace whose contents changed.

A document surface's freshness SHALL NOT depend on where in the workspace its document lies. For a document beneath a change directory the workspace watcher supplies the notification; for any other markdown document — a capability specification beneath `openspec/specs/`, or a file anywhere else in the browse root — a document watch supplies it, per the `document-watch` capability. Both deliver the same guarantee to the surface, and a surface SHALL NOT be required to know which mechanism notified it.

A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT alter the rendered document when the document's bytes are unchanged. (This narrows a previous contract, under which such a refresh was required to be wholly unobservable when the bytes were unchanged. That is no longer correct: a file rewritten with identical bytes has a **new modification time**, and the header reports modification time — see *Change Identity Header in the Detail Pane*, "Last changed". The protection this clause exists to give — an undisturbed reader, no loading indicator, no repaint of the document — is unchanged; what narrows is its scope, from the whole pane to the document within it.) When such a refresh fails to read the document, the surface SHALL continue to display the content it already holds rather than replacing it with an error. A read the user initiated by selecting a document retains its existing loading and error presentation.

The cost of the unfiltered subscription SHALL remain bounded by this guarantee: a refresh that changes neither the document's bytes nor its modification time SHALL do no work beyond the read itself, and one that changes only the modification time SHALL NOT re-render the document. Where two mechanisms both notify a surface about one document — possible for a document beneath a change directory, per the *Independent of the Workspace Watcher* requirement in the `document-watch` capability — the surface SHALL coalesce the resulting refreshes, and the duplicate SHALL remain unobservable under the clause above.

#### Scenario: Tree updates when new change appears

- **WHEN** a new change directory is created on disk in a registered workspace
- **THEN** the new change appears as a child of that workspace in the tree

#### Scenario: Detail pane updates when shown file is edited

- **WHEN** the detail pane is currently rendering an artifact's markdown
- **AND** that markdown file is modified on disk
- **THEN** the detail pane re-renders with the updated content

#### Scenario: Every document surface updates when its file is edited

- **WHEN** a document is rendered in the file browser's preview region or in a reader window
- **AND** that file is modified on disk
- **THEN** that surface re-renders with the updated content

#### Scenario: Freshness does not depend on where the document lives

- **WHEN** a document surface is rendering a markdown file that lies outside `openspec/changes/`
- **AND** that file is modified on disk
- **THEN** the surface re-renders with the updated content, exactly as it would for a change artifact

#### Scenario: Reading position survives a refresh the user did not initiate

- **WHEN** a document surface is rendering a document and the user has scrolled away from the top
- **AND** that document's file is modified on disk while the user's selection is unchanged
- **THEN** the surface renders the updated content
- **AND** the reading position is preserved — the surface neither scrolls to the top nor scrolls back to a section or task the user selected in the tree earlier
- **AND** no loading indicator is presented

#### Scenario: Refresh with unchanged content is not observable

- **WHEN** a document surface is rendering a document
- **AND** a filesystem change elsewhere triggers a refresh whose read returns content identical to what is displayed, with an unchanged modification time
- **THEN** the rendered output, the reading position, and the loading indicator are all unchanged

#### Scenario: A duplicate notification for one document is not observable

- **WHEN** a document surface is rendering a document beneath a change directory, for which both the workspace watcher and a document watch deliver a notification for one edit
- **THEN** the resulting refreshes are coalesced
- **AND** the rendered output, the reading position, and the loading indicator show no additional disturbance

#### Scenario: A modification-time-only change updates the header and nothing else

- **WHEN** a document surface is rendering a document
- **AND** a refresh returns content identical to what is displayed but a newer modification time
- **THEN** the header's last-changed label updates
- **AND** the rendered document is not re-rendered
- **AND** the reading position is preserved and no loading indicator is presented

#### Scenario: Refresh is not conditioned on the workspace the notification names

- **WHEN** a document surface is rendering a document belonging to one tracked workspace
- **AND** a filesystem-change notification arrives naming a different tracked workspace
- **THEN** the surface still re-reads its document and renders the current on-disk content

#### Scenario: Failed background read preserves the displayed content

- **WHEN** a document surface is rendering a document
- **AND** a refresh the user did not initiate fails to read that document, because its file was removed, became unreadable, or was caught mid-write
- **THEN** the surface continues to display the content it already loaded
- **AND** no error state replaces it

#### Scenario: Failed selection read still reports the error

- **WHEN** the user selects a document whose file cannot be read
- **THEN** the document surface presents its error state

#### Scenario: Tree updates when change is archived on disk

- **WHEN** a change directory is moved from `openspec/changes/<id>/` to `openspec/changes/archive/<id>/`
- **THEN** the change is removed from the tree

#### Scenario: Artifact row flips to present when its file is created inside an existing change

- **WHEN** a change directory already exists and is tracked by the watcher (for example because `openspec new change` previously wrote only its `.openspec.yaml`)
- **AND** a subsequent on-disk write creates one of the four artifact files (`proposal.md`, `design.md`, `tasks.md`, or a `specs/<capability>/spec.md`) inside that change directory
- **THEN** the corresponding artifact row in the tree re-renders as present (full opacity, interactive) within the watcher's debounce window
- **AND** the row reaches its present state on the first refresh the frontend performs after that write — no further on-disk edit or user action is required to flip the row

#### Scenario: Instance-row task progress updates when a checkbox is toggled

- **WHEN** an instance row (or, for a singleton logical change, the flattened row) is rendered in the tree with a task-progress meter
- **AND** an on-disk edit to that change's `tasks.md` flips a task line's checkbox between `- [ ]` and `- [x]`
- **THEN** the row's task-progress meter re-renders with its fill width reflecting the new completion ratio within the watcher's debounce window
- **AND** the new fill is visible on the first refresh the frontend performs after that edit — no further edit, focus change, or window action is required to surface it

#### Scenario: Section completion glyph and auto-collapse update when the last task in a section is toggled

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an on-disk edit to `tasks.md` toggles the last incomplete task in that section from `- [ ]` to `- [x]`
- **THEN** the Section row's trailing `✓` glyph and the Section's auto-collapsed rendering both appear within the watcher's debounce window
- **AND** both are visible on the first refresh the frontend performs after that edit

### Requirement: Window State Persistence

The main window's position, size, and maximised state SHALL persist across application restarts.

#### Scenario: Window size restored after relaunch

- **WHEN** the user resizes the main window to a non-default dimension
- **AND** quits and relaunches the application
- **THEN** the main window opens at the previously set size and position

### Requirement: Read-Only Viewer

In v1, the application SHALL NOT modify any spec file as a result of user interaction with the rendered content.

#### Scenario: Task checkboxes are not interactive

- **WHEN** the detail pane is rendering a `tasks.md` containing markdown task checkboxes
- **THEN** clicking a rendered checkbox does not modify the underlying file

### Requirement: Per-Instance Divergence Label

For every `ChangeInstance` that is not on the repository's default branch, the application SHALL compute and display at most one divergence label by comparing the instance's change directory contents against the default-branch instance of the same logical change. The labels are:

- `[diverged]` — the change exists in both the default-branch instance and the non-default instance, but the file contents differ at the byte level.
- `[stale]` — the change is archived on the default branch (under `openspec/changes/archive/`) but is still active in the non-default instance.

If the change does not exist on the default branch at all, or if no default branch is known, or if the contents are identical, the instance SHALL display no divergence label.

The label is a property of an **instance**, and is displayed where instances are named: on the instance's entry in the change header's instance switcher (see *Instance Switcher in the Change Header*) when the change has several rendered instances, and on the change row's detail line when the change has exactly one (see *Two-Line Sole-Change-Row Layout*). A multi-instance change row itself SHALL show no divergence label, because the row names no instance.

Two instances SHALL be recognised as instances of the same logical change whether or not they are archived, and whether or not their archive directories carry a date prefix. An archived instance's directory is named `<YYYY-MM-DD>-<id>` in ordinary use and `<id>` in the legacy un-dated form; the logical change it belongs to is identified by `<id>` in both cases. Comparing an active instance against an archived one SHALL therefore key on the change's bare identifier, never on the archive directory's raw name — keying the two forms differently makes the `[stale]` label unreachable for every dated archive directory, which is the form that occurs in practice.

#### Scenario: Diverged content gets the diverged label

- **WHEN** an instance on a non-default branch has different content under `openspec/changes/<name>/` than the default-branch instance of the same logical change
- **THEN** the instance's switcher entry (or, for a singleton, its change row) displays the `[diverged]` label

#### Scenario: Stale-vs-archive gets the stale label

- **WHEN** the default-branch instance of a logical change is in `openspec/changes/archive/<name>/`
- **AND** a non-default instance of the same logical change is in `openspec/changes/<name>/` (still active)
- **THEN** the non-default instance's switcher entry (or, for a singleton, its change row) displays the `[stale]` label

#### Scenario: Stale label fires against a dated archive directory

- **WHEN** the default-branch instance of a logical change `add-thing` is archived at `openspec/changes/archive/2026-09-05-add-thing/`
- **AND** a non-default instance is still active at `openspec/changes/add-thing/`
- **THEN** the non-default instance's switcher entry (or, for a singleton, its change row) displays the `[stale]` label
- **AND** the date prefix on the archive directory does not prevent the two instances from being recognised as the same logical change

#### Scenario: Branch-only change gets no label

- **WHEN** a logical change has no instance on the default branch (it was created only on a feature branch)
- **THEN** every non-default instance displays no divergence label

#### Scenario: Identical content gets no label

- **WHEN** a non-default instance has byte-identical content to the default-branch instance of the same logical change
- **THEN** the non-default instance's switcher entry (or, for a singleton, its change row) displays no divergence label

#### Scenario: No default branch produces no labels

- **WHEN** the repository has no detected default branch
- **THEN** no instance of any logical change in that repository displays a divergence label

### Requirement: Change-Row Completion Glyph

For change rows that surface task progress — the flat-workspace change row (`FlatChangeNode`) and the logical-change row (`LogicalChangeRow`), see *Two-Line Sole-Change-Row Layout* — when every parsed task in the change is complete (`totalTasks > 0` and `completedTasks === totalTasks`), the row SHALL render a trailing `Check` glyph in the row's meta cluster. On the logical-change row, the in-progress task-progress meter is hidden at 100% (see the *Task Progress Meter* requirement in `visual-identity`) and the `Check` occupies the meta position the meter would otherwise hold. When at least one task is incomplete, or when the change has no tasks at all, the row SHALL NOT render the trailing `Check` glyph.

The `Check` glyph SHALL NOT appear in the row's leading slot on either row type. Pre-existing leading-position completion markers (specifically the leading `Check` on `FlatChangeNode` rendered when all tasks were done) SHALL be removed.

#### Scenario: Flat-change row gets a trailing tick when all tasks complete

- **WHEN** a flat-workspace change row is rendered for a change with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph
- **AND** no `Check` glyph appears in the row's leading slot

#### Scenario: Instance row gets a trailing tick when all tasks complete

- **WHEN** a per-instance change row is rendered for an instance with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph and renders no task-progress meter (the meter is hidden at 100%)
- **AND** the glyph sits where the meter would otherwise be, between the leading meta and the modification-time element

#### Scenario: Rows without complete tasks have no trailing tick

- **WHEN** a flat-change row or instance row is rendered for a change with at least one incomplete task, or for a change with no tasks at all
- **THEN** the row's meta cluster contains no `Check` glyph
- **AND** the leading slot also contains no `Check` glyph

### Requirement: Settings Entrypoint in Sidebar Footer

The Settings entrypoint SHALL be rendered as a labeled row pinned to the bottom of the tree-navigation (left) pane. The row SHALL contain an icon and the visible text label "Settings". The row SHALL remain visible regardless of the scroll position of the workspace tree above it.

Clicking the row SHALL toggle the right pane between the workspace-tree's detail view and the Settings view, preserving the existing toggle semantics (a second click while Settings is open returns the right pane to its prior detail-view target).

The row SHALL convey its current state visually: when Settings is open in the right pane, the row SHALL render in an active treatment distinct from its idle state, mirroring the established active-affordance vocabulary already used elsewhere in the application chrome.

The Settings entrypoint SHALL NOT be rendered as a floating button overlaying the master-detail surface. No Settings affordance SHALL appear in the top-right corner of the application window.

#### Scenario: Footer row is visible at startup

- **WHEN** the user opens the main window
- **THEN** a row labeled "Settings" with an icon is rendered at the bottom of the left sidebar
- **AND** no floating Settings button is rendered in the top-right corner of the window

#### Scenario: Footer row stays pinned while the tree scrolls

- **WHEN** the workspace tree contains more rows than fit in the sidebar's height
- **AND** the user scrolls the tree to its midpoint or end
- **THEN** the Settings row remains visible at the bottom of the sidebar without scrolling out of view

#### Scenario: Clicking the row opens Settings

- **WHEN** the user clicks the Settings row while the right pane is showing a detail view
- **THEN** the right pane swaps to the Settings view
- **AND** the Settings row renders in its active state

#### Scenario: Clicking the row again closes Settings

- **WHEN** the user clicks the Settings row while Settings is already open in the right pane
- **THEN** the right pane returns to its prior detail-view target
- **AND** the Settings row returns to its idle state

#### Scenario: Selecting a tree node while Settings is open closes Settings

- **WHEN** Settings is open in the right pane
- **AND** the user clicks a renderable tree node (instance, artifact, spec, section, or task)
- **THEN** the right pane swaps to that node's detail view
- **AND** the Settings row returns to its idle state

### Requirement: Proposal Title Extraction

The title of a change SHALL be extracted from its `proposal.md` as follows. The parser SHALL skip ignorable preamble at the top of the document: blank lines, one leading YAML frontmatter block (when the first content line is exactly `---`, through its closing `---`), and HTML comment blocks (`<!--` through `-->`, single- or multi-line). The first content line after the preamble SHALL yield a title only when it is a level-1 Markdown heading — a single `#` followed by whitespace and non-empty text after trimming leading whitespace. An optional case-insensitive `Proposal:` prefix SHALL be stripped from the heading text, and the result trimmed. Any other first content line — a deeper heading such as `## Why`, body text, or an unterminated preamble block — SHALL yield no title, and the parser SHALL NOT examine any further line of the document. A change with no extractable title SHALL continue to be labelled by its change ID wherever titles are displayed (sidebar rows, archive browser, dashboard). A missing or unreadable `proposal.md` SHALL yield no title.

#### Scenario: Title on the first line parses as before

- **WHEN** a `proposal.md` begins with `# Add User Auth` on line 1
- **THEN** the extracted title is "Add User Auth"
- **AND** a legacy `# Proposal: Add User Auth` first line also yields "Add User Auth"

#### Scenario: Title found below ignorable preamble

- **WHEN** a `proposal.md` opens with blank lines, a YAML frontmatter block, or HTML comments (in any combination), followed by `# Add User Auth`
- **THEN** the extracted title is "Add User Auth"

#### Scenario: Template-faithful proposal yields no title

- **WHEN** a `proposal.md` follows the spec-driven template and its first content line is `## Why`
- **THEN** no title is extracted (never "Why")
- **AND** the change's rows display its change ID

#### Scenario: Non-heading first content line yields no title

- **WHEN** the first content line after the preamble is body text, a deeper heading, or `#` without a following space
- **THEN** no title is extracted and no later line of the document is considered
- **AND** an h1 appearing only later in the body (for example inside a fenced code block) is never mistaken for the title

### Requirement: Two-Line Sole-Change-Row Layout

A change row — the **sole row for its change**, since the tree renders exactly one row per change (see *Workspace Tree Hierarchy*) — SHALL render across two stacked lines within a single selectable row. Exactly two row types are change rows:

- a **logical-change row** — one row per git logical change, whatever its rendered instance count; and
- a **flat-workspace change row** — a `ChangeData` row rendered directly under a non-git workspace node.

Repo-group and workspace header rows are excluded and SHALL remain single-line. No other row type exists in the tree.

**Line 1 (primary).** Line 1 SHALL display the change's `proposal.md` title when one is extractable (see *Proposal Title Extraction*) — falling back, for a git singleton, to the logical change name, and for a flat-workspace change row, to its directory name. When a git singleton's line 1 shows the proposal title, the row SHALL expose the logical change name via its hover tooltip so the directory identity stays recoverable. The label SHALL render with slightly heavier weight than the tree's meta text so it reads as the row's heading, and SHALL own the full row width — no trailing branch chip or status meta shares the line — except for the favorite toggle's reserved trailing slot (see *Change-Row Favorite Toggle*); it SHALL ellipsize against that slot when it exceeds the available width. Line 1 carries no worktree identity, swatch, or colour tint on its text.

**Line 2 (detail).** Line 2 SHALL render at the tree's dense meta type tier, visually subordinate, and SHALL be indented to begin at line 1's text origin (past the chevron) so it reads as belonging to the row above it. Line 2 SHALL place worktree identity on its leading edge and status on its trailing edge:

- **Leading edge.** For a git singleton row the leading edge SHALL show the instance's branch name as an outlined chip (per *visual-identity → Outlined Chip Badges*) tinted to the owning workspace's palette colour — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour. When the branch is not known (detached HEAD, bare worktree), the chip SHALL show the worktree folder basename instead. For a **logical-change row with two or more rendered instances** the leading edge SHALL instead show an **instance-count chip** (`2 worktrees`) in the same outlined-chip treatment, untinted, and SHALL name no branch: the instances' branches are named in the change header's instance switcher (see *Instance Switcher in the Change Header*). A flat-workspace change row has no git worktree identity; in its place the leading edge SHALL show the change's identifier (`changeId`), the same identifier the row shows today.
- **Status (trailing).** Line 2 SHALL carry the row's existing status elements, with their existing presence rules, on its trailing edge. For a **git singleton row** these are the task-progress meter while work is in progress or the completion ✓ when every task is complete (per *Change-Row Completion Glyph* and the *Task Progress Meter* requirement in `visual-identity`), the relative modification time, and the divergence label when present (per *Per-Instance Divergence Label*). For a **multi-instance logical-change row** the status elements are those of the change's **default instance** (see *Workspace Tree Hierarchy*), and no divergence label is shown on the row: divergence is a per-instance property and is shown per instance in the change header's switcher. For a **flat-workspace change row** the only status element is the completion ✓ when every task is complete; a flat-workspace row carries no progress meter, modification time, or divergence label.

**Workspace-colour rail.** A sole change row SHALL tint its inline-start border — the 2px slot the selection bar occupies — with the owning workspace's palette colour, so each change reads as belonging to its workspace and the colour ties the row to its branch chip top-to-bottom. While the row is selected the selection bar (the 2px `--accent` border, per *visual-identity → Tree Row Selection Model*) SHALL take precedence and replace the rail; the rail SHALL reappear when the row is deselected. A workspace with no configured palette colour renders no rail. Header rows and the other excluded row types do not render the rail.

**One interaction unit.** The two lines SHALL form a single interaction unit: one click target that selects the change and one selection unit. The selection treatment (the 2px `--accent` inline-start bar plus its tint wash) and the hover wash SHALL span both lines. A change row has no disclosure chevron: it is a leaf. The favorite toggle (see *Change-Row Favorite Toggle*) is the row's only nested control; activating it SHALL NOT select the change.

#### Scenario: Git singleton renders its proposal title on the first line

- **WHEN** a git logical change has exactly one instance and its `proposal.md` yields a title
- **THEN** line 1 shows that title across the full row width, in a slightly heavier weight than line 2
- **AND** the label is not truncated by any branch or status element on the same line; only the favorite toggle's reserved trailing slot bounds it
- **AND** the row's hover tooltip carries the logical change name

#### Scenario: Git singleton without an extractable title falls back to the change name

- **WHEN** a git logical change has exactly one instance and its `proposal.md` is missing or yields no title
- **THEN** line 1 shows the logical change name, exactly as before

#### Scenario: Branch appears on the detail line as a workspace-tinted chip

- **WHEN** a git singleton instance's worktree is on a named branch
- **THEN** line 2 shows the branch name as an outlined chip on its leading edge, with chip text and border tinted to the owning workspace's palette colour (a contrast-safe shade)
- **AND** line 2 shows the task-progress meter (or completion ✓) and relative modification time on its trailing edge

#### Scenario: Detached-HEAD singleton shows the folder basename only

- **WHEN** a git singleton instance's worktree is not on a named branch
- **THEN** line 2's worktree-identity segment shows the worktree folder basename alone, with no branch name

#### Scenario: Flat-workspace change row uses two lines with a meta-only detail line

- **WHEN** a change is rendered as a flat-workspace change row under a non-git workspace node
- **THEN** line 1 shows the change's title (or its change-id when no title is present)
- **AND** line 2 shows the change's `changeId` on its leading edge and the completion ✓ (when complete) on its trailing edge, with no branch, worktree folder, progress meter, modification time, or divergence label

#### Scenario: Multi-instance change renders one row with an instance count

- **WHEN** a logical change has two or more rendered instances
- **THEN** it renders as exactly one two-line row
- **AND** line 2's leading edge shows an untinted instance-count chip naming the number of worktrees and no branch
- **AND** line 2's trailing edge shows the default instance's progress meter (or completion ✓) and relative modification time, and no divergence label

#### Scenario: Completed sole change row shows its completion glyph on the detail line

- **WHEN** a sole change row's change has at least one task and every task is complete
- **THEN** line 2's trailing edge shows the completion ✓ in place of the progress meter

#### Scenario: Selection and hover span both lines of a sole change row

- **WHEN** a sole change row is selected, or the pointer hovers over either of its two lines
- **THEN** the selection bar and tint (or the hover wash) cover both lines as one contiguous row
- **AND** a click anywhere on either line — outside the favorite toggle — selects the change and updates the detail pane

#### Scenario: Workspace-colour rail marks each change row

- **WHEN** a sole change row is rendered for a workspace that has a configured palette colour
- **AND** the row is not selected
- **THEN** the row's inline-start border (the selection-bar slot) is tinted to that workspace's palette colour
- **AND** every change row under the same workspace shares that colour, matching the workspace's top-level swatch

#### Scenario: Selection bar overrides the rail

- **WHEN** a sole change row that is showing its workspace-colour rail becomes selected
- **THEN** the inline-start border renders the 2px `--accent` selection bar instead of the workspace colour
- **AND** the workspace-colour rail reappears once the row is deselected

### Requirement: Archive Entrypoint in Sidebar Footer

The Archive entrypoint SHALL be rendered as a labeled row in the bottom region of the tree-navigation (left) pane, directly above the Settings entrypoint (see *Settings Entrypoint in Sidebar Footer*). The row SHALL contain an icon and the visible text label "Archive". The row SHALL remain visible regardless of the scroll position of the workspace tree above it.

Clicking the row SHALL toggle the right pane between its prior detail target and the **Archive view** (see the *Archive View* requirement in the `archive-browser` capability), preserving the same toggle semantics the Settings entrypoint uses: a second click while the Archive view is open returns the right pane to its prior detail target, and selecting a renderable tree node, opening Settings, or opening the Dashboard closes the Archive view.

The row SHALL convey its current state visually: when the Archive view is open in the right pane, the row SHALL render in an active treatment distinct from its idle state, mirroring the active-affordance vocabulary used by the Settings and Dashboard entrypoints.

The Archive entrypoint SHALL NOT be rendered as a floating button overlaying the master-detail surface, and SHALL NOT display a count badge — no archive content is computed until the view is opened.

#### Scenario: Archive row is visible in the sidebar footer

- **WHEN** the user opens the main window
- **THEN** a row labeled "Archive" with an icon is rendered in the sidebar footer, above the "Settings" row
- **AND** no floating Archive button is rendered over the master-detail surface
- **AND** the Archive row displays no count badge

#### Scenario: Clicking the row opens the Archive view

- **WHEN** the user clicks the Archive row while the right pane is showing a detail target
- **THEN** the right pane swaps to the Archive view
- **AND** the Archive row renders in its active state

#### Scenario: Clicking the row again closes the Archive view

- **WHEN** the user clicks the Archive row while the Archive view is already open
- **THEN** the right pane returns to its prior detail target
- **AND** the Archive row returns to its idle state

#### Scenario: Selecting a tree node closes the Archive view

- **WHEN** the Archive view is open in the right pane
- **AND** the user selects a renderable tree node (instance, artifact, spec, section, or task)
- **THEN** the right pane swaps to that node's detail view
- **AND** the Archive row returns to its idle state

### Requirement: Workspace Tree Keyboard Navigation

The workspace tree SHALL be fully operable from the keyboard as a WAI-ARIA tree with a roving tabindex over its two levels: the tree occupies exactly one position in the window's Tab order, and within it a single current row carries focus, movable with the keyboard. Keyboard activation SHALL reuse the same selection contract as pointer clicks — a change row activated by keyboard navigates exactly as a click on it does, and a top-level row, whose click is disclosure-only, remains disclosure-only. Switching between a change's artifacts is not a tree operation: it is done in the change header's tab strip, whose keyboard model *Artifact Tab Strip in the Change Header* defines.

#### Scenario: Tree is a single Tab stop with a roving current row

- **WHEN** the user presses Tab from the control preceding the tree (or Shift+Tab from the control following it)
- **THEN** focus lands on the tree's current row — the last row focused in this session, or the first visible row if none — rather than entering every row in sequence
- **AND** pressing Tab again moves focus out of the tree to the next control in the window's Tab order

#### Scenario: Arrow keys traverse visible rows

- **WHEN** the tree has focus and the user presses ArrowDown or ArrowUp
- **THEN** focus moves to the next or previous visible row in rendered order, crossing workspace boundaries, without wrapping at either end
- **AND** the newly focused row scrolls into view if it is outside the sidebar's viewport

#### Scenario: Home and End jump to the extremes

- **WHEN** the tree has focus and the user presses Home or End
- **THEN** focus moves to the first or last visible row of the tree

#### Scenario: ArrowRight and ArrowLeft drive disclosure and parent jumps

- **WHEN** the user presses ArrowRight on a closed top-level row
- **THEN** the row opens for the session, identically to a chevron click
- **WHEN** the user presses ArrowRight on an open top-level row
- **THEN** focus moves to its first change row
- **WHEN** the user presses ArrowLeft on an open top-level row
- **THEN** the row closes
- **WHEN** the user presses ArrowLeft on a change row, or on a closed or leaf top-level row
- **THEN** focus moves to the row's top-level row, or does not move when the row is already top-level

#### Scenario: Enter and Space activate the current row

- **WHEN** the user presses Enter or Space on a change row
- **THEN** the row is selected and the detail pane renders exactly what a pointer click on that row would render — the change's default artifact of its default instance
- **WHEN** the user presses Enter or Space on a top-level row with changes
- **THEN** the row's disclosure toggles, identically to a chevron click
- **WHEN** the user presses Enter or Space on an empty top-level row
- **THEN** the row is selected and the detail pane shows the workspace file browser, as a click would

#### Scenario: Debounced follow-focus opens content without per-keystroke reads

- **WHEN** keyboard focus comes to rest on a change row and remains there for a short settle delay (approximately 150 ms)
- **THEN** the detail pane renders that change's default artifact as if the row had been activated
- **WHEN** focus passes over change rows more quickly than the settle delay (for example while an arrow key is held down)
- **THEN** no intermediate change's artifact is loaded or rendered
- **WHEN** keyboard focus rests on a top-level row
- **THEN** the detail pane does not change

#### Scenario: First-letter typeahead

- **WHEN** the tree has focus and the user types a printable character
- **THEN** focus moves to the next visible row after the current one whose label starts with that character, comparing case-insensitively and wrapping past the end of the tree
- **AND** if no visible row label starts with that character, focus does not move

#### Scenario: Tree rows expose ARIA tree semantics

- **WHEN** the tree is rendered
- **THEN** the container exposes `role="tree"`, every row exposes `role="treeitem"` with `aria-level` of `1` for a top-level row and `2` for a change row, top-level rows with changes expose `aria-expanded` reflecting their disclosure state, the selected row exposes `aria-selected="true"`, and each top-level row's change rows are wrapped in a `role="group"` container
- **AND** no row exposes `aria-disabled`: every rendered row responds to activation

#### Scenario: Focus survives the focused row disappearing

- **WHEN** a tree refresh (for example a filesystem cache event) removes the change row that currently holds keyboard focus
- **THEN** focus falls back to that change's top-level row, rather than being lost to the document body

#### Scenario: Keyboard focus movement does not re-render the whole tree

- **WHEN** the user moves keyboard focus between rows
- **THEN** only the rows whose visual state changed re-render; unaffected subtrees are not re-rendered

### Requirement: Shell Keyboard Operability

The browsing shell around the tree SHALL be keyboard-operable: split-pane dividers MUST be focusable and resizable from the keyboard, the Settings and Archive panes MUST be dismissible with Escape, and every keyboard-focusable control in the shell MUST show a visible focus indicator when focused via keyboard, using the visual-identity spec's keyboard-focus recipe.

#### Scenario: Dividers resize from the keyboard

- **WHEN** a split-pane divider receives keyboard focus and the user presses ArrowLeft or ArrowRight
- **THEN** the adjacent pane resizes by a fixed step per keypress, respecting the same minimum-width limits as a pointer drag, and the divider exposes `role="separator"` with `aria-valuenow`, `aria-valuemin`, and `aria-valuemax` reflecting the current and permitted sizes

#### Scenario: Escape dismisses Settings and Archive

- **WHEN** the Settings pane or the Archive pane is open and the user presses Escape (with no text input focused that consumes it)
- **THEN** the open pane closes and the detail pane returns to what it previously displayed

#### Scenario: Focusable controls show visible keyboard focus

- **WHEN** any focusable control in the sidebar, archive view, graph rail, or settings view receives focus via keyboard
- **THEN** it renders a visible focus indicator per the visual-identity keyboard-focus recipe
- **AND** focus styles use `:focus-visible` so pointer clicks do not paint focus rings

### Requirement: Working-Tree Status Indicators

The tree pane SHALL surface git working-tree status for git-backed repositories
through two indicator families, leaving non-git (flat) workspaces unchanged.

On each repository node, when the repository's dirty rollup is set, the tree
SHALL render a whole-repo **dirty** indicator; when the repository additionally
has uncommitted specs, the tree SHALL render a **distinct** specs-uncommitted
indicator alongside it, so that an uncommitted source file is visually
distinguishable from an uncommitted spec. Both indicators SHALL be absent when
the repository is clean.

On each change-instance row, the tree SHALL render a commit-state chip when the
instance's spec commit state is `Modified` or `Untracked`, positioned alongside
the existing divergence chip. A `Committed` instance SHALL render no such chip.

#### Scenario: Repo with an uncommitted spec shows both rollup indicators

- **WHEN** a repository node renders and the repository has a worktree with an
  untracked or modified change directory
- **THEN** the node shows the whole-repo dirty indicator
- **AND** the node shows the distinct specs-uncommitted indicator

#### Scenario: Repo dirty only from non-spec files shows one indicator

- **WHEN** a repository is dirty solely from files outside `openspec/`
- **THEN** the node shows the whole-repo dirty indicator
- **AND** the node does not show the specs-uncommitted indicator

#### Scenario: Clean repo shows no indicators

- **WHEN** a repository and all its worktrees are clean
- **THEN** the repository node shows neither indicator

#### Scenario: Untracked instance shows a commit-state chip

- **WHEN** a change-instance row renders for a worktree whose copy of the change
  is untracked
- **THEN** the row shows an "untracked" commit-state chip beside the divergence
  chip

#### Scenario: Committed instance shows no commit-state chip

- **WHEN** a change-instance row renders for a worktree whose copy of the change
  is fully committed
- **THEN** the row shows no commit-state chip

#### Scenario: Flat workspace is unaffected

- **WHEN** a non-git (flat) workspace renders in the tree
- **THEN** no working-tree status indicators are shown for it

### Requirement: Artifact Reads Are Confined to Registered Workspaces

Reading an OpenSpec artifact's markdown SHALL be authorized only when the workspace it is read from is a registered (or registry-discovered) workspace, and a caller-supplied workspace that is not in the registry SHALL be refused rather than read, even when the requested path resolves to a real `openspec/changes/…` file on disk. This authorization SHALL be applied in addition to the existing path-traversal guard (which keeps the resolved file within the workspace's `openspec/changes/` subtree): the traversal guard bounds *where within a workspace* a read may reach, and this requirement bounds *which workspaces* may be read at all. The workspace SHALL be matched by its canonical path against the registry's known workspace folders using the same canonicalization the registry keys on, and the check SHALL be enforced at the shared application boundary so it holds for every frontend and transport that can read artifacts.

#### Scenario: An artifact read against an unregistered workspace is refused

- **WHEN** an artifact-read is requested for a workspace path that is not a registered or registry-discovered workspace
- **THEN** the read is refused with an error
- **AND** no file under that path is read, even if an `openspec/changes/.../<artifact>.md` file exists there

#### Scenario: An artifact read against a registered workspace succeeds

- **WHEN** an artifact-read is requested for a change in a registered workspace
- **THEN** the artifact's markdown is returned as before, subject to the existing path-traversal guard

#### Scenario: The confinement holds across transports

- **WHEN** an artifact-read is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same registered-workspace requirement applies, because it is enforced at the shared application boundary

### Requirement: Wide Block Containment

No single block of a rendered artifact SHALL widen the document column or cause the detail pane to scroll horizontally. Every block-level element whose natural width exceeds the content column — a GFM table, a fenced code block, display mathematics, a diagram held at its legibility floor, or an image — SHALL be contained within its own bounds and SHALL scroll horizontally inside those bounds when its content cannot shrink to fit. The prose around such a block SHALL remain fixed in place while the block is scrolled.

Containment SHALL NOT introduce vertical clipping or a vertical scrollbar on the contained block: content that overhangs the block's line box vertically (a summation limit, a subscript, a descender) SHALL remain fully visible.

#### Scenario: A wide table scrolls within its own block

- **WHEN** an artifact contains a GFM table whose columns cannot fit the content column at their readable widths
- **THEN** the table scrolls horizontally within its own block
- **AND** the document column and the detail pane do not scroll horizontally
- **AND** the surrounding prose keeps its position while the table is scrolled

#### Scenario: A contained block never grows a vertical scrollbar

- **WHEN** an artifact contains display mathematics or a table contained by this requirement
- **THEN** the containing block shows no vertical scrollbar
- **AND** no vertical overhang of the content is clipped

### Requirement: Mermaid Diagram Rendering

The detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability (`mermaid` here, `svg` in the *SVG Fence Rendering* requirement, `math` in the *Mathematical Notation Rendering* requirement) SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

A rendered diagram SHALL derive its colours and fonts from the application's design tokens (see the *Design Token Layer* and *Typography System* requirements in the `visual-identity` capability) so that it reads as part of the same surface as the surrounding prose in both the light and dark schemes. This obligation extends to colours the diagram engine derives on its own for values the application does not map explicitly: the application SHALL inform the engine of the active scheme so that every derived colour is derived in the direction of that scheme, rather than under an assumed light palette. Diagram text SHALL remain legible against every filled surface the engine draws — including alternating table-row fills such as entity-relationship attribute rows, whose fills SHALL come from the design tokens' surface colours. When the operating system colour scheme changes while a diagram is visible, the diagram SHALL re-render so its colours follow the active scheme.

A diagram whose natural width exceeds the detail pane's content width SHALL scale down to fit — but only to a **legibility floor**. With an authored diagram label size $$f_{\text{label}}$$ and a fit-to-pane scale $$s_{\text{fit}}$$, the diagram SHALL render at

$$s_{\text{render}} = \max\left(s_{\text{fit}},\ \frac{f_{\min}}{f_{\text{label}}}\right), \qquad f_{\min} = 10\,\text{px}$$

so rendered label text never falls below $$f_{\min}$$. A diagram held at the floor is wider than the pane and SHALL scroll horizontally within its own block per the *Wide Block Containment* requirement. Every successfully rendered diagram — scaled, floor-held, or natural size — SHALL remain openable in the maximized view described by the *Maximized Figure View* requirement.

A `mermaid` fence whose content is not valid diagram source SHALL degrade gracefully: the detail pane SHALL present the fence's raw source together with a quiet indication that the diagram could not be rendered, SHALL NOT blank or crash the pane, and SHALL NOT surface the diagram engine's own error graphic. The rest of the artifact SHALL render normally. Diagram rendering SHALL run under a strict security posture so that diagram source cannot inject active content (scripts or click-through handlers) into the application.

#### Scenario: A valid mermaid fence renders as a diagram

- **WHEN** an artifact contains a fenced code block with the `mermaid` info string and valid diagram source
- **THEN** the detail pane renders it as a graphical diagram
- **AND** the raw mermaid source text is not shown

#### Scenario: Other fenced code blocks are unaffected

- **WHEN** an artifact contains a fenced code block in another language (for example `rust` or `ts`)
- **THEN** it renders as syntax-highlighted source as before
- **AND** it is not treated as a diagram

#### Scenario: An invalid mermaid fence degrades to source

- **WHEN** an artifact contains a `mermaid` fence whose content is not valid diagram source
- **THEN** the detail pane shows the fence's raw source
- **AND** shows a quiet indication that the diagram could not be rendered
- **AND** the rest of the artifact still renders
- **AND** the diagram engine's default error graphic is not shown

#### Scenario: Diagrams follow the design tokens and colour scheme

- **WHEN** a diagram is rendered
- **THEN** its colours and font derive from the application's design tokens rather than the diagram engine's stock palette
- **AND** the diagram engine is informed of the active colour scheme, so colours it derives on its own follow that scheme
- **AND** when the operating system switches between light and dark while the diagram is visible, the diagram re-renders with the active scheme's tokens

#### Scenario: Entity-relationship attribute rows stay legible in the dark scheme

- **WHEN** an artifact contains an `erDiagram` fence whose entities carry attributes and the dark colour scheme is active
- **THEN** every attribute row's fill comes from the design tokens' surface colours
- **AND** the row text remains legible against its row fill
- **AND** no row renders as a near-white fill beneath near-white text

#### Scenario: Diagram source cannot inject active content

- **WHEN** a `mermaid` fence contains content that attempts to embed a script or a click-through handler
- **THEN** the rendered diagram contains no active content
- **AND** no script from the diagram source executes

#### Scenario: A moderately wide diagram scales down to fit

- **WHEN** an artifact contains a diagram whose natural width exceeds the pane's content width by less than the legibility floor allows
- **THEN** it is displayed scaled down to fit the pane, with no horizontal scrolling
- **AND** its rendered label text is no smaller than the legibility floor

#### Scenario: A very wide diagram stops shrinking at the legibility floor

- **WHEN** an artifact contains a diagram so wide that fitting the pane would render its label text below the legibility floor
- **THEN** the diagram renders at the floor scale instead of fitting the pane
- **AND** it scrolls horizontally within its own block while the pane and document column do not
- **AND** a control to open it in the maximized view is available on it

### Requirement: SVG Fence Rendering

The detail pane SHALL render a fenced code block whose info string is `svg` as an image rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability — including `xml` — SHALL continue to render as syntax-highlighted source, unchanged; the `mermaid` and `math` info strings remain governed by the *Mermaid Diagram Rendering* and *Mathematical Notation Rendering* requirements respectively. Image rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `svg` fences as code text.

The fence body SHALL be presented through an image context (an `<img>` element whose source is derived from the fence body) so that active content is structurally impossible: scripts, event handlers, and references to external resources appearing in the fence body SHALL NOT execute or load. The renderer SHALL NOT inject the fence body into the host document's live DOM. This obligation holds at every displayed size, including within the maximized view.

The fence body SHALL be validated as an SVG document before display. A fence whose body is not well-formed SVG SHALL degrade gracefully: the detail pane SHALL present the fence's raw source together with a quiet indication that the image could not be rendered, SHALL NOT blank or crash the pane, and the rest of the artifact SHALL render normally. The same source fallback SHALL apply if the image context itself fails to load the derived source.

A valid fence body SHALL be normalized before display, and only in the following ways:

- A missing `xmlns` declaration on the root `svg` element SHALL be injected (it is mandatory for a standalone SVG document but routinely omitted by authors), and its absence alone SHALL NOT be treated as invalid SVG.
- When the root element lacks usable absolute `width` AND lacks usable absolute `height` — both must be missing or unusable, not merely one — but declares a `viewBox`, the width and height SHALL be derived from the viewBox extents at one user unit per CSS pixel; the displayed image SHALL be capped at the pane's content width while preserving its aspect ratio. When exactly one of `width` or `height` is authored and usable, both SHALL be left as authored: the image context SHALL derive the missing dimension from the viewBox ratio natively.
- When the root `svg` element does not already declare a `color`, the application's text design token (see the *Design Token Layer* requirement in the `visual-identity` capability) SHALL be set as the root's `color`, so that `currentColor` occurrences resolve to it through ordinary CSS inheritance within the image document; when the operating system colour scheme changes while such an image is visible, it SHALL re-render with the newly active token. A `color` the author declared — on the root or any descendant — SHALL take precedence, and the fence body SHALL NOT otherwise be rewritten.

Colours the author wrote explicitly SHALL NOT be altered: the renderer SHALL NOT invert, matte, or otherwise repaint fence content for the active scheme beyond the root `color` injection above. When the SVG document contains a root-level `<title>` element, its text SHALL be used as the image's alternative text; otherwise a generic alternative text SHALL identify the image as an embedded SVG.

A rendered image SHALL be openable in the maximized view described by the *Maximized Figure View* requirement, so that an image capped at the pane's content width remains legible.

#### Scenario: A valid svg fence renders as an image

- **WHEN** an artifact contains a fenced code block with the `svg` info string and a well-formed SVG body
- **THEN** the detail pane renders it as an image
- **AND** the raw SVG source text is not shown

#### Scenario: Other fenced code blocks are unaffected

- **WHEN** an artifact contains a fenced code block in another language (for example `xml` or `rust`)
- **THEN** it renders as syntax-highlighted source as before
- **AND** it is not treated as an image

#### Scenario: An invalid svg fence degrades to source

- **WHEN** an artifact contains an `svg` fence whose body is not well-formed SVG
- **THEN** the detail pane shows the fence's raw source
- **AND** shows a quiet indication that the image could not be rendered
- **AND** the rest of the artifact still renders

#### Scenario: Fence content cannot inject active content

- **WHEN** an `svg` fence body contains a script element, an event-handler attribute, or a reference to an external resource
- **THEN** no script executes and no external resource is fetched
- **AND** the fence body is not inserted into the host document's live DOM

#### Scenario: A naïve fence still renders correctly

- **WHEN** an `svg` fence body omits the `xmlns` declaration and declares only a `viewBox` with no `width` or `height`
- **THEN** it renders as an image sized from the viewBox extents
- **AND** it is not treated as invalid SVG

#### Scenario: currentColor follows the active colour scheme

- **WHEN** an `svg` fence body uses `currentColor` for fills or strokes without declaring its own `color`
- **THEN** those fills and strokes render with the application's text design token
- **AND** when the operating system switches between light and dark while the image is visible, it re-renders with the newly active token
- **AND** colours the author wrote explicitly — including `currentColor` resolved under an author-declared `color` — are unchanged in both schemes

#### Scenario: A rendered image offers the maximized view

- **WHEN** an artifact contains an `svg` fence that renders as an image
- **THEN** a control to open it in the maximized view is available on it
- **AND** the image's inline size and aspect ratio are unchanged by the presence of that control

### Requirement: Maximized Figure View

A figure the detail pane has rendered successfully — a `mermaid` diagram (see the *Mermaid Diagram Rendering* requirement) or an `svg` image (see the *SVG Fence Rendering* requirement) — SHALL be openable in a **maximized view**: a surface presented above the entire application window in which that single figure can be enlarged, reduced, and moved. A fence that degraded to its source, and a diagram whose rendering has not yet completed, SHALL NOT offer the maximized view, because neither has a figure to show.

**Affordance.** Each maximizable figure SHALL present a control that opens the maximized view. The control SHALL be operable by keyboard as well as by pointer (see the *Shell Keyboard Operability* requirement). On a figure that is rendered below its natural size — scaled down to fit the pane, or held at the legibility floor of the *Mermaid Diagram Rendering* requirement — the control SHALL be visible at rest on every device, because a reduced figure is exactly the one whose reader needs the escape to full size. On a device that reports no hover capability it SHALL be rendered visibly at rest regardless, and on a device whose primary pointer is coarse it SHALL present an enlarged hit area, per the *Essential Controls Are Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirements in the `touch-input` capability. The figure's inline presentation SHALL be unchanged by the presence of the control: a figure still fits the detail pane's width while reading (or scrolls within its block at the legibility floor), and the maximized view is an addition to that default rather than a replacement for it.

**Initial scale.** The maximized view SHALL open with the figure fully visible — scaled so that neither dimension exceeds the surface's content area, with the scale taken from whichever axis constrains it more. For a surface of extents $$W_v \times H_v$$ with padding $$p$$, displaying content of extents $$W_c \times H_c$$:

$$s_{\text{fit}} = \min\left(\frac{W_v - 2p}{W_c},\ \frac{H_v - 2p}{H_c}\right)$$

**Zoom.** The maximized view SHALL support continuous zoom by wheel and by two-contact pinch, and SHALL provide explicit controls to return to the fit scale and to display the figure at actual size. Zoom driven by a pointer gesture SHALL be anchored at that pointer: the point of the figure beneath the pointer SHALL remain beneath it as the scale changes. Scale SHALL be bounded — never reduced below the fit scale (or actual size, whichever is smaller) and never increased beyond a fixed ceiling — so the figure can neither be lost in the surface nor enlarged without limit.

**Pan.** While the figure exceeds the surface's content area it SHALL be movable by dragging, and dragging SHALL be driven by pointer input so that a mouse, a touch contact, and a pen all move it through the same path (see the *Drag Interactions Accept Pointer Input* requirement in the `touch-input` capability).

**Fidelity.** An enlarged figure SHALL be re-rendered at the size at which it is displayed, rather than by magnifying a fixed-resolution rendering of it. Enlarging SHALL NOT degrade a figure's sharpness, in either the diagram path or the image path.

**Security posture is preserved.** The maximized view SHALL NOT relax the rendering guarantees of either path. An `svg` fence SHALL continue to be presented through an image context and SHALL NOT be injected into the host document's live DOM at any scale, and a `mermaid` diagram SHALL continue to be rendered under the strict security posture its own requirement specifies. The maximized view SHALL offer no means of editing, exporting, or otherwise writing the figure, consistent with the *Read-Only Viewer* requirement.

**Colour scheme.** While the maximized view is open the figure SHALL follow the active colour scheme exactly as it does inline, re-rendering when the operating system switches between light and dark. That re-render SHALL preserve the current scale and position, so a scheme change does not displace what the reader is looking at.

**Dismissal.** The maximized view SHALL be dismissable by the Escape key, by an explicit close control, and by activating the surface outside the figure. Dismissal SHALL return the reader to the artifact with its scroll position unchanged. Escape SHALL dismiss only the maximized view: any Settings or Archive pane open behind it SHALL remain open, and a second Escape SHALL be required to dismiss that (see the *Archive Entrypoint in Sidebar Footer* and *Settings Entrypoint in Sidebar Footer* requirements).

**The maximized view is ambient view state.** It SHALL NOT be part of the Address, the URL, or navigation history (see the `view-routing` capability), consistent with how side-pane visibility is treated by the *Side-Pane Visibility Toggles* requirement. Navigating to a different artifact SHALL close it.

A change to the artifact's content on disk while the view is open SHALL also close it. The maximized view SHALL NOT continue to present a figure rendered from source the artifact no longer contains, and SHALL NOT re-open itself on the reader's behalf. Holding it open across a reparse would require identifying one figure within an artifact across an edit to that artifact — which this capability deliberately does not do, for the same reason the view carries no Address. Closing is the honest outcome: the artifact behind it has already updated in place per the *Reactive Updates from Filesystem* requirement, and the affordance to maximize the new figure is immediately available.

#### Scenario: A rendered diagram can be maximized

- **WHEN** the detail pane has rendered a `mermaid` fence as a diagram
- **THEN** a control to maximize that diagram is available on it
- **AND** activating the control opens the diagram in a surface above the application window
- **AND** the diagram is initially shown fully visible within that surface

#### Scenario: A rendered svg image can be maximized

- **WHEN** the detail pane has rendered an `svg` fence as an image
- **THEN** a control to maximize that image is available on it
- **AND** activating the control opens the image in the same maximized surface the diagram path uses

#### Scenario: A reduced figure shows its maximize control at rest

- **WHEN** the detail pane renders a diagram scaled below its natural size — fit to the pane or held at the legibility floor
- **THEN** the maximize control on that figure is visible without hovering or focusing it
- **AND** a figure rendered at its natural size continues to reveal the control on hover or keyboard focus

#### Scenario: A degraded fence offers no maximized view

- **WHEN** an artifact contains a `mermaid` fence whose content is not valid diagram source, or an `svg` fence whose body is not well-formed SVG
- **THEN** the fence's raw source is shown with its quiet indication as before
- **AND** no maximize control is offered on it

#### Scenario: Zoom is anchored at the pointer

- **WHEN** the reader zooms in with the pointer resting over a particular node of a maximized diagram
- **THEN** that node remains beneath the pointer as the scale increases
- **AND** the rest of the figure expands around it

#### Scenario: Scale is bounded at both ends

- **WHEN** the reader zooms out repeatedly in the maximized view
- **THEN** the figure stops reducing once it is fully visible and does not shrink further
- **AND** zooming in repeatedly stops at the maximum scale rather than continuing without limit

#### Scenario: An enlarged image stays sharp

- **WHEN** the reader enlarges a maximized `svg` image well beyond its inline size
- **THEN** the image is re-rendered at the displayed size
- **AND** it is not shown as a magnified low-resolution rendering

#### Scenario: An enlarged figure can be moved

- **WHEN** a maximized figure has been enlarged beyond the surface's content area
- **THEN** dragging it moves the visible region
- **AND** a mouse drag, a touch drag, and a pen drag each move it the same way

#### Scenario: Escape dismisses only the maximized view

- **WHEN** the Archive view is open, an artifact is rendered behind it, and a figure in that artifact has been maximized
- **THEN** pressing Escape closes the maximized view
- **AND** the Archive view remains open
- **AND** pressing Escape again closes the Archive view

#### Scenario: Maximizing does not change the address

- **WHEN** the reader maximizes a figure and then dismisses it
- **THEN** the Address, the URL, and the navigation history are unchanged throughout
- **AND** the artifact's scroll position is unchanged when the view is dismissed

#### Scenario: Navigating away closes the maximized view

- **WHEN** a figure is maximized and the reader selects a different artifact in the tree
- **THEN** the maximized view closes
- **AND** the newly selected artifact is rendered in the detail pane with no figure maximized

#### Scenario: A live edit closes the maximized view rather than showing superseded source

- **WHEN** a figure is maximized and the artifact's file changes on disk so that its content is reparsed
- **THEN** the maximized view closes
- **AND** it never displays a figure rendered from source the artifact no longer contains
- **AND** the artifact behind it shows the reparsed content with its maximize affordance available

#### Scenario: A scheme change preserves scale and position

- **WHEN** a diagram is maximized and enlarged, and the operating system switches between light and dark
- **THEN** the diagram re-renders with the active scheme's design tokens
- **AND** its scale and visible region are unchanged

#### Scenario: Maximizing preserves the image path's inertness

- **WHEN** an `svg` fence whose body contains a script element or an event-handler attribute is maximized
- **THEN** the fence body is still not inserted into the host document's live DOM
- **AND** no script executes and no external resource is fetched at any scale

### Requirement: Link Handling in Rendered Artifacts

A link click inside markdown rendered by the shared markdown renderer — change artifacts, archived artifacts, and workspace file-browser previews alike — SHALL never navigate the application's webview: every anchor activation SHALL be intercepted and dispatched by link class, any class without a defined behaviour SHALL be inert, and activation paths that bypass the renderer's click handling (such as the webview's native context menu or link drag-out) SHALL be denied by a shell-level navigation guard that permits only the application's own origin.

An absolute link with an `http` or `https` scheme SHALL open in the system default browser, and a `mailto:` or `tel:` link SHALL open via the operating system's default handler, in each case leaving the application view unchanged.

A relative link to a non-markdown file SHALL be resolved against the directory of the markdown file being viewed — after stripping any fragment and query and percent-decoding the path exactly once — and opened with the operating system's default handler for the target's type; for an `.html` mockup that is the default browser, which resolves the mockup's sibling assets (stylesheets, scripts, images) itself. The target MAY live anywhere inside the authorized root; it is not confined to the change directory the linking artifact belongs to. This boundary is deliberately wider than the `openspec/changes/` subtree that confines artifact reads (see *Artifact Reads Are Confined to Registered Workspaces*): the open operation reads and returns no file content — its effect is limited to asking the OS to display an allow-listed document inside a folder the user brought into the application.

Opening SHALL be authorized at the shared application boundary before any opener is invoked:

- The root SHALL be authorized by the same rule that authorizes file browsing (see *Browsing Is Confined to Registered Workspaces* in the `workspace-file-browser` capability): a registered or registry-discovered workspace, or a repository main worktree accepted because a worktree of that repository is registered. An unauthorized root SHALL be refused before any path is resolved.
- The canonicalised target SHALL be contained within the canonicalised authorized root — so a `..` traversal (encoded or not) or a symlink pointing outside the root is refused rather than opened.
- The target SHALL match a case-insensitive allow-list of document types — initially `.html`, `.htm`, `.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp`, `.avif`, `.css`, `.pdf`, `.txt`, `.json`, `.csv` — and directories SHALL be refused. Executable and script targets are therefore never opened: following a link SHALL NOT be able to execute a file.

The frontend SHALL NOT hold a general open-URL or open-path capability; the only open operation reachable from rendered content is this validated one.

Relative links to markdown files (matched case-insensitively) SHALL be inert in v1, reserved for future in-app navigation. Links with any other scheme (including `javascript:` and `file:`) SHALL be inert. A **fragment-only** link is dispatched by the `document-outline` capability's *Fragment Links Resolve Within the Document* requirement: it scrolls within the rendered document when its fragment names a heading, and fails quietly as a dangling link when it does not; it never navigates the application. Inert links SHALL carry a visual affordance distinguishing them from openable links, so a dead link reads as policy rather than breakage; a fragment link that resolves to a heading is not inert and SHALL NOT carry it.

A click whose target does not exist or is refused SHALL produce a quiet indication that the link could not be opened, SHALL NOT navigate or blank the pane, and SHALL leave the rendered artifact fully usable.

Opening files is a desktop-frontend concern; other frontends degrade per their own capability specs, and the raw artifact markdown returned by the backend is unchanged by this requirement.

#### Scenario: An external link opens in the system browser

- **WHEN** the user clicks a link with an `http` or `https` URL in a rendered artifact
- **THEN** the URL opens in the system default browser
- **AND** the application view does not navigate away

#### Scenario: A relative HTML mockup link opens externally

- **WHEN** a change's `proposal.md` contains a relative link to `./mockups/login.html` and that file exists
- **THEN** clicking the link opens the mockup via the operating system's default handler for HTML
- **AND** the detail pane still shows the rendered proposal

#### Scenario: A mockup outside the change directory opens

- **WHEN** an artifact links to an `.html` file that resolves inside the authorized root but outside the linking change's directory
- **THEN** clicking the link opens the file via the operating system's default handler

#### Scenario: A fragment- or query-suffixed file link opens the file

- **WHEN** an artifact links to `./mockups/login.html#hero` or `./mockups/login.html?v=2`, or to a target whose name is percent-encoded (such as `./my%20mockup.html`)
- **THEN** the underlying file resolves and opens as if linked plainly

#### Scenario: A link escaping the root is refused

- **WHEN** an artifact's relative link resolves outside the authorized root — via `..` traversal (plain or percent-encoded) or via a symlink inside the root whose target lies outside it
- **THEN** nothing is opened
- **AND** a quiet indication is shown that the link could not be opened
- **AND** the pane neither navigates nor blanks

#### Scenario: An executable or directory target is refused

- **WHEN** an artifact links to an executable or script file (such as `./run.sh`, `./setup.command`, or `./tool.exe`) or to a directory (including an `.app` bundle)
- **THEN** nothing is opened or executed
- **AND** a quiet indication is shown that the link could not be opened

#### Scenario: The open operation refuses an unauthorized root

- **WHEN** the open operation is invoked with a root that is neither a registered or registry-discovered workspace nor a repository main worktree accepted by the browsing rule
- **THEN** it is refused with an error before any path is resolved or opened

#### Scenario: A relative markdown link is inert

- **WHEN** the user clicks a relative link to a markdown file in a rendered artifact, regardless of extension casing (`./notes.md`, `./NOTES.MD`)
- **THEN** nothing opens and the application view does not change

#### Scenario: A script-scheme link is inert

- **WHEN** a rendered artifact contains a link with a `javascript:` or `file:` href
- **THEN** clicking it executes nothing, opens nothing, and does not navigate the webview

#### Scenario: Bypassing the click handler cannot navigate the app

- **WHEN** a link is activated through a path that bypasses the renderer's click handling — the webview context menu's open-link action, or dragging the link
- **THEN** the application webview does not navigate away from the app UI

#### Scenario: A dangling link fails quietly

- **WHEN** the user clicks a relative link whose target file does not exist
- **THEN** a quiet indication is shown that the link could not be opened
- **AND** the rendered artifact remains fully usable

#### Scenario: A fragment link scrolls within the document

- **WHEN** the user clicks a fragment-only link whose fragment names a heading of the rendered artifact
- **THEN** the artifact scrolls to that heading
- **AND** the application view, the address and the history are unchanged

### Requirement: Mathematical Notation Rendering

The detail pane SHALL render mathematical notation as typeset formulas using double-dollar and fence delimiters only: a double-dollar-delimited expression (`$$…$$`) standing alone as its own paragraph — in either its single-line or multi-line block form — SHALL render as display (block) mathematics, a double-dollar expression embedded within surrounding prose SHALL render as inline mathematics, and a fenced code block whose info string is `math` SHALL render as display mathematics rather than as syntax-highlighted source. A single-dollar-delimited span (`$…$`) SHALL NOT be treated as mathematics: it SHALL render as literal text, dollar signs included, so prose that mentions dollar amounts can never be silently consumed and re-typeset as a formula. Mathematics rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend SHALL continue to present mathematical source as plain text.

Dollar delimiters SHALL NOT be recognised inside code spans or fenced code blocks (other than the `math` fence itself), so a literal dollar sign in backticked text — for example a `\\wsl$\<distro>` path — is never parsed as mathematics. A dollar sign with no valid closing delimiter SHALL render as a literal dollar sign.

Inline mathematics SHALL render at a size visually harmonized with the surrounding prose: the rendering engine's default enlargement relative to the surrounding font SHALL be overridden so a formula sits on the same optical line as the words around it without inflating the line's height. Display mathematics SHALL NOT be vertically clipped by its own block: limits, subscripts, and descenders SHALL remain fully visible, and the block SHALL NOT display a vertical scrollbar. Display mathematics wider than the pane's content width SHALL scroll horizontally within its own block rather than widening the artifact (see the *Wide Block Containment* requirement).

Rendered mathematics SHALL inherit the surrounding text colour, so it follows the active colour scheme in both light and dark without any repainting or re-rendering machinery. Rendered mathematics SHALL carry a machine-readable representation (MathML) alongside the visual output so assistive technology can consume it. Rendering SHALL work without network access: the mathematics engine and its assets are part of the application bundle.

Invalid input SHALL degrade gracefully and locally: a double-dollar-delimited expression that is not valid mathematical source SHALL present its raw source in place with a quiet visual indication of the error, while the rest of the artifact renders normally; a `math` fence whose body cannot be rendered SHALL likewise present the fence's raw source with a quiet visual indication that the formula could not be rendered. Neither case SHALL blank or crash the pane.

Mathematics rendering SHALL run under a non-trusting posture so mathematical source cannot inject active content: commands that would emit hyperlinks, external references, or scripts (for example `\href`) SHALL NOT produce live links, fetch external resources, or execute.

#### Scenario: Inline math renders within prose via double dollars

- **WHEN** an artifact contains a double-dollar expression such as `$$O(n \log n)$$` embedded mid-sentence
- **THEN** the detail pane renders it as typeset inline mathematics flowing with the surrounding text
- **AND** the raw LaTeX source is not shown
- **AND** its rendered size sits with the surrounding prose rather than enlarged above it

#### Scenario: Display math renders as a block

- **WHEN** an artifact contains a double-dollar-delimited expression standing alone as its own paragraph (single-line or multi-line block form) or a fenced code block with the `math` info string
- **THEN** the detail pane renders it as display mathematics in its own block
- **AND** a formula wider than the pane's content width scrolls horizontally within that block without widening the artifact

#### Scenario: Prose dollar amounts are never mathematics

- **WHEN** an artifact contains prose with multiple single-dollar amounts, such as "the plan costs $50 per seat and $60 with add-ons"
- **THEN** the sentence renders exactly as written, dollar signs and spacing intact
- **AND** no part of it is typeset as mathematics

#### Scenario: Display math is never vertically clipped

- **WHEN** an artifact contains display mathematics with under-limits or deep subscripts, such as a summation with a bound beneath it
- **THEN** every limit, subscript, and descender is fully visible
- **AND** the block shows no vertical scrollbar

#### Scenario: Dollar signs in code are never math

- **WHEN** an artifact contains dollar signs inside a code span or a fenced code block in another language — for example `` `\\wsl$\Ubuntu\home` `` or `` `releases/${tag}.md` ``
- **THEN** they render as literal dollar signs, unchanged
- **AND** a dollar sign in prose with no valid closing delimiter renders as a literal dollar sign

#### Scenario: Invalid inline math degrades in place

- **WHEN** an artifact contains a double-dollar-delimited expression whose content is not valid mathematical source
- **THEN** the detail pane presents that expression's raw source in place with a quiet indication of the error
- **AND** the rest of the artifact still renders normally

#### Scenario: An invalid math fence degrades to source

- **WHEN** an artifact contains a `math` fence whose body cannot be rendered
- **THEN** the detail pane shows the fence's raw source
- **AND** shows a quiet indication that the formula could not be rendered
- **AND** the rest of the artifact still renders

#### Scenario: Math source cannot inject active content

- **WHEN** a mathematical expression attempts to emit a hyperlink, an external reference, or a script (for example via `\href`)
- **THEN** the rendered output contains no live link and no active content
- **AND** no external resource is fetched and no script executes

#### Scenario: Math follows the colour scheme

- **WHEN** rendered mathematics is visible and the operating system switches between light and dark
- **THEN** the mathematics renders with the active scheme's surrounding text colour in both schemes

### Requirement: Change-Row Favorite Toggle

Exactly three row types are **favoritable rows** — the rows that aggregate a whole change: the flattened singleton logical-change row, the multi-instance logical-change disclosure parent row, and the flat-workspace change row. (This set is distinct from the "change-aggregating rows" of the *Change-Row Completion Glyph* requirement, which includes per-instance rows; a favorite always attaches to the change, never to one worktree's instance.) Every favoritable row SHALL render a favorite toggle (a star glyph) in a reserved slot at the extreme trailing edge of the row's primary line. On rows that already render trailing meta on that line (for example the multi-instance parent's instance-count badge), the slot sits after the existing meta, and because the slot is reserved, revealing or hiding the star SHALL NOT shift any other row content. Instance child rows beneath a multi-instance parent SHALL NOT render the toggle.

The toggle SHALL present two visual states: while the change is not a favorite, an outline star in the faint ink colour (`--text-faint`) that is hidden at rest and revealed while the row is hovered or holds the tree's roving focus; while the change is a favorite, a solid star in the accent ink (`--accent`) that is always visible — at rest, on hover, and while the row is selected. The filled star carries no glow and is the sole indicator of favorite status; no other badge, label, or row treatment conveys it. (The solid accent star is sanctioned by the *Accent Color* and *Outlined Chip Badges* censuses in the `visual-identity` capability, as modified by this change.)

On a device that reports no hover capability, the outline star SHALL be visible at rest rather than hidden, because neither hover nor the keyboard chord below is available to reveal it (see the *Essential Controls Are Discoverable Without Hover* requirement in the `touch-input` capability). Its reserved slot SHALL continue to prevent any other row content from shifting.

Activating the toggle SHALL flip the change's favorite state and SHALL NOT select the row, change the tree's selected-node state, or alter the detail pane — mirroring the disclosure chevron's contract that a nested row control never triggers row selection. The toggle SHALL NOT join the tab order: the tree retains its roving-focus, single-Tab-stop keyboard model, and the toggle itself is never focusable. The nested button SHALL expose `aria-pressed` and an accessible label, and the favorite state SHALL additionally be conveyed at the treeitem level (in the row's accessible name or description), so assistive technology that flattens nested-control state still announces it.

Favorite state SHALL additionally be togglable by keyboard: Cmd+D (macOS) / Ctrl+D (Windows, Linux) toggles the favorite state of the focused favoritable row, with the same binding active in the served web UI, where it SHALL suppress the browser's native bookmark shortcut. The chord SHALL take precedence over first-letter typeahead: a keypress carrying the platform command modifier SHALL NOT move typeahead focus (see the *Workspace Tree Keyboard Navigation* requirement). When the focused row is not a favoritable row, the binding SHALL have no effect.

#### Scenario: Hover reveals the outline star on a non-favorite row

- **WHEN** the pointer hovers a favoritable row whose change is not a favorite
- **THEN** an outline star appears in the reserved slot at the trailing edge of the row's primary line
- **AND** the star is not visible on that row at rest
- **AND** no other content on the row shifts when the star appears

#### Scenario: Outline star is visible at rest on a touch device

- **WHEN** the served web UI is loaded on a device that reports no hover capability
- **AND** a favoritable row's change is not a favorite
- **THEN** the outline star is visible on that row at rest
- **AND** activating it flips the change's favorite state
- **AND** no other content on the row is shifted by the star's presence

#### Scenario: Keyboard focus reveals the outline star

- **WHEN** a favoritable row whose change is not a favorite receives the tree's roving focus
- **THEN** the outline star is visible on that row
- **AND** the toggle itself is not focusable and the tree's single Tab stop is preserved

#### Scenario: Favorite rows show a persistent filled star

- **WHEN** a favoritable row's change is a favorite
- **THEN** the row renders a solid `--accent` star, with no glow, in its reserved trailing slot at rest, on hover, and while selected

#### Scenario: Toggling never selects the row

- **WHEN** the user clicks the star on a favoritable row while another node is selected
- **THEN** the change's favorite state flips
- **AND** the tree's selected node is unchanged
- **AND** the detail pane's contents are unchanged

#### Scenario: Instance child rows carry no star

- **WHEN** a multi-instance logical change is expanded
- **THEN** none of its instance child rows renders a favorite toggle
- **AND** the disclosure parent row renders the toggle for the logical change

#### Scenario: Cmd/Ctrl+D toggles the focused change row

- **WHEN** a favoritable row has keyboard focus
- **AND** the user presses Cmd+D (macOS) / Ctrl+D (Windows, Linux)
- **THEN** that change's favorite state flips
- **AND** focus and selection are unchanged
- **AND** typeahead does not move focus in response to the chord's letter

#### Scenario: Cmd/Ctrl+D elsewhere is inert

- **WHEN** a non-favoritable row (an artifact, section, task, capability spec, instance, or top-level row) has keyboard focus
- **AND** the user presses Cmd+D (macOS) / Ctrl+D (Windows, Linux)
- **THEN** no favorite state changes anywhere in the tree

### Requirement: Favorite-First Change Ordering

Within each top-level group — the logical changes under a Repo group, and the changes under a flat workspace — the tree SHALL render favorite changes before non-favorite changes, preserving the existing name order within each partition. The partition SHALL introduce no divider, section header, or count row: the filled star on each floated row is the only indicator of the grouping.

This ordering governs the favoritable rows of the tree pane only. It SHALL NOT reorder top-level rows, instance child rows (which keep their existing order), artifact nodes (fixed order per the *Workspace Tree Hierarchy* requirement), the Archive view (date-ordered), or the Dashboard's feeds. The terminal frontend is outside this capability and keeps the shared core's order.

#### Scenario: Favorite changes float to the front of their group

- **WHEN** a Repo group contains changes `alpha`, `mid`, and `zulu`, and `zulu` is a favorite
- **THEN** the group renders its change rows in the order `zulu`, `alpha`, `mid`
- **AND** no divider or header row separates `zulu` from `alpha`

#### Scenario: Name order is preserved within each partition

- **WHEN** a group contains favorites `delta` and `bravo` and non-favorites `charlie` and `alpha`
- **THEN** the rows render in the order `bravo`, `delta`, `alpha`, `charlie`

#### Scenario: Unfavoriting returns a row to its name-order slot

- **WHEN** the user removes the favorite state from a floated change row
- **THEN** the row re-renders in its name-order position among the non-favorite rows within the same top-level group

#### Scenario: Ordering applies per group, not across groups

- **WHEN** changes are favorites in two different top-level groups
- **THEN** each group floats only its own favorites to its own front
- **AND** the order of the top-level rows themselves is unchanged

### Requirement: Favorite Identity and Persistence

A favorite SHALL be keyed on the logical change's position-independent identity: for a repo-group change, the repository identity plus the change directory name; for a flat-workspace change, the workspace identity plus the change directory name. The favorite SHALL therefore be unaffected by singleton↔multi-instance promotion, by which worktrees currently host the change, and by tree position.

Favorite state SHALL persist across application restarts in application settings, alongside the collapse-state overrides — never inside any workspace's `openspec/` tree. A settings file written by a version predating this feature SHALL load cleanly with an empty favorites set. Writes SHALL be coalesced so rapid toggling does not write a settings file per intermediate state, and the persisted state eventually reflects the final toggled positions.

A persisted favorite whose change is not currently rendered — because the change is archived, its workspace is unregistered, or no change by that name exists — SHALL be inert: it is ignored while unmatched and applies again if a matching change reappears. The application is not required to garbage-collect inert entries. Favorite state is ambient view preference: it SHALL NOT be part of the Address, the URL, or navigation history, and navigating SHALL NOT change any favorite.

In the served web UI, favorite state is backed by the serving machine's application settings, shared with the desktop app and every connected client of that machine, consistent with how the collapse-state overrides behave. A concurrently connected client reflects another client's toggle the next time it loads the tree (for example a page reload); no push update or same-session convergence is required.

#### Scenario: Favorites survive a restart

- **WHEN** the user favorites a change and quits and relaunches the application
- **THEN** the change renders as a favorite, floated to the front of its group, without further user action

#### Scenario: Favorite survives singleton-to-multi promotion

- **WHEN** a favorited singleton logical change gains a second worktree instance
- **THEN** the resulting multi-instance disclosure parent row renders as a favorite
- **AND** removing all but one instance leaves the flattened row still a favorite

#### Scenario: Favorite goes inert on archive and returns on reappearance

- **WHEN** a favorited change is archived on disk
- **THEN** the change leaves the tree (per the *Workspace Tree Hierarchy* requirement) and its favorite entry has no visible effect
- **AND** when a change with the same identity is active again, its row renders as a favorite

#### Scenario: Pre-feature settings file loads cleanly

- **WHEN** the application starts against a settings file with no favorites field
- **THEN** settings load successfully
- **AND** the tree renders with no favorites and all other persisted preferences intact

#### Scenario: Rapid toggling coalesces writes

- **WHEN** the user toggles the same change's favorite state several times in rapid succession
- **THEN** the persisted state eventually reflects the final position
- **AND** the application does not write a settings file for every intermediate state

#### Scenario: Web clients share the serving machine's favorites

- **WHEN** a change is favorited in the desktop app on the serving machine
- **AND** a browser client subsequently loads the served web UI
- **THEN** the web tree renders that change as a favorite, floated to the front of its group

#### Scenario: Navigation does not alter favorites

- **WHEN** the user follows any address, including Back/Forward
- **THEN** no favorite state changes
- **AND** no favorite state appears in the address

### Requirement: Change Identity Header in the Detail Pane

While the detail pane's target is an OpenSpec artifact, the pane SHALL render a **change header** above the artifact's markdown, naming the change the artifact belongs to and carrying the change's navigation. The header applies to the artifact target only: the commit detail view, the Dashboard, the workspace file browser, and the Settings view each carry their own header and SHALL be unaffected. The **Archive reader** renders this same header, in a read-only form, as the `archive-browser` capability's *Read-Only Artifact Navigation* requirement specifies; it carries no header of its own.

**Rows.** The header is composed of stacked rows in a fixed order: the **identity row** this requirement defines; then the **instance switcher** (see *Instance Switcher in the Change Header*), rendered only when the change has more than one rendered instance; then the **artifact tab strip** (see *Artifact Tab Strip in the Change Header*). In the read-only form a leading row precedes the identity row, carrying the control that returns to the archive listing together with the archived change's date and title. Every row lies inside the single sticky, measured element this requirement's *Persistence while reading*, *Clearance* and *Anchoring* clauses describe, so those clauses bind the whole header and a scroll anchor clears all of it.

**Content.** The header SHALL display the change's **directory name** — the `openspec/changes/<name>` folder name, which is the identifier a user hands to external tooling — rendered verbatim and in full, with no truncation, ellipsis, or transformation. It SHALL NOT substitute the change's `proposal.md` title, which the tree already shows (see *Two-Line Sole-Change-Row Layout*) and which is not the change's filesystem identity. Following the name, the header SHALL show the owning worktree's branch as an outlined chip (per *visual-identity → Outlined Chip Badges*). When the artifact belongs to a flat (non-git) workspace, or the worktree's branch is otherwise not known, no chip SHALL be rendered and the header SHALL show the name alone. An **archived** change SHALL render no chip: it has no live worktree, and the worktree path its artifact is read from routinely hosts other, active changes whose branch was never the archived change's.

**Branch chip colour.** The branch chip SHALL be **tinted to the owning workspace's palette colour** — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour — so the artifact under the header reads as belonging to the workspace it came from. The workspace whose colour applies is the one owning the worktree the artifact was read from, which is the same worktree whose branch the chip names; no other workspace's colour SHALL be substituted. When the owning workspace has no configured palette colour, the chip SHALL render in the neutral ink it renders in today, and SHALL NOT fall back to an arbitrary or derived colour.

The header's chip and the tree's chip naming the same branch of the same change SHALL render **identically** — the same tint, weight, and treatment (see the *Two-Line Sole-Change-Row Layout* requirement, which specifies the tree's chip). The two surfaces are visible simultaneously, so a single value SHALL NOT be presented two ways. This equivalence is a property of the rendered result and SHALL hold for every palette colour and for the untinted case, so that changing how one surface renders the chip cannot leave the other behind.

**Last changed.** The header SHALL report **when the artifact currently rendered last changed**, as an interval elapsed since that moment, expressed in relative terms (for example `just now`, `9m ago`, `12d ago`) rather than as an absolute clock time. The detail pane is already refreshed live (see *Reactive Updates from Filesystem*), so this value does not report whether the view is current; it reports how long the artifact has stood.

The label SHALL use the **same relative-time vocabulary** as every other surface in the application that presents an elapsed time — the tree's change-row modification time (see *Two-Line Sole-Change-Row Layout*) and the Dashboard's relative archive time (see the `dashboard` capability). The header and the tree are visible simultaneously and routinely describe the same change, so one kind of value SHALL NOT be spelled two ways on one screen. This equivalence is a property of the rendered result and SHALL hold at every tier of the vocabulary, so that changing how one surface words an interval cannot leave the others behind.

The value SHALL be the modification time of **the artifact's own file** — not of the change's directory, and not of any sibling artifact. A write to `tasks.md` SHALL NOT be reported as a change to the `proposal.md` on screen, because the two are edited independently and reporting the directory's newest write would be wrong in exactly the case a reader is most likely to be watching.

Where **no** modification time is available for the artifact's file, the header SHALL render no label at all, and SHALL NOT substitute a default, derived, or epoch time in its place. The artifact itself SHALL still be displayed: an unreadable timestamp is not a failed read, and a reader SHALL NOT lose the document because the application could not date it. This mirrors the branch chip, which is likewise absent rather than defaulted when there is no branch to name.

The value SHALL be a filesystem modification time, and the header SHALL claim no more of it than that. Any operation that writes the file sets it, including a clone, a checkout, or a branch switch — so on a freshly cloned repository every artifact SHALL report having last changed at the time of that clone, regardless of when it was genuinely edited. This SHALL hold **uniformly for every artifact**, active and archived alike: no class of artifact SHALL substitute a different source for this value, because the property belongs to modification times in general and not to any one class, and an exception carved for one class would imply the others are trustworthy in a way they are not.

The label SHALL **advance without user action** for as long as the artifact remains on screen, so a reader who has not navigated away is never shown an interval that stopped counting when the pane was painted. It SHALL be updated at a cadence no finer than the smallest unit it displays. An elapsed interval that would be negative — a file whose modification time lies in the future, which clock skew, restored archives, and network filesystems all produce — SHALL be presented as the present moment rather than as a future time. Whatever advances the label SHALL NOT outlive the artifact it described.

The label SHALL occupy a **constant width**, sized to the widest text it can display, so that neither its advancing nor a change of unit alters the layout of the header. The change name shares a single flex row with the branch chip and yields to width pressure by re-wrapping mid-identifier; a label whose box changed size as it advanced would therefore re-lay-out the change name on a timer, unprompted. This is the same defect the *Confirmation* clause below forbids for a click-driven width change, arriving from a trigger the reader did not initiate at all.

**Copy on click.** The change name is a **control**. A single primary click on it SHALL place exactly that name on the clipboard. What is copied SHALL be the name alone — never the branch chip's text, never the last-changed label, and never any surrounding whitespace. The same click SHALL also select the name atomically, so the selection serves as immediate confirmation of exactly what was copied.

The application SHALL perform the clipboard write itself. (This supersedes the previous contract, under which the application performed no clipboard write and the user completed the copy with the platform's own gesture; that contract was adopted under constraints of the tree pane, which do not apply to the detail pane.) Where the asynchronous Clipboard API is not exposed — a non-loopback bind on a plain-HTTP origin is not a secure context, see the `web-ui` capability — the application SHALL still copy, using a synchronous copy over the selection it has just made. The two mechanisms SHALL NOT be chained such that a failure of the first is what triggers the second, because the synchronous mechanism is only permitted inside the originating user gesture and awaiting the asynchronous one ends it.

**Confirmation.** The header SHALL confirm the outcome. A successful copy SHALL be indicated visually and announced to assistive technology; a refused copy SHALL be distinguished from a successful one, and SHALL leave the name selected so the platform's own copy shortcut still completes the action. Confirmation SHALL NOT change the layout of the header — no label substitution, no added or removed glyph — because the name shares a flex row with the branch chip and may wrap, so any width change would move the row on every copy. Confirmation SHALL revert on its own, and SHALL NOT outlive the artifact it described.

**Keyboard.** The change name SHALL be reachable by keyboard as a single tab stop within the detail pane, SHALL expose an accessible name describing the copy action and the value, and SHALL be activated by Enter and by Space, performing the same copy as a click. It SHALL show the application's standard focus indicator. No global keyboard chord SHALL be introduced for this; in particular the platform's own copy shortcut SHALL retain its native meaning everywhere. The tree's roving-focus, single-Tab-stop model SHALL be unaffected.

**Persistence while reading.** The header SHALL remain visible while the artifact's content scrolls, so the change's identity is answerable at any scroll position rather than only at the top of the document.

**Clearance of the native titlebar strip.** In the native desktop window on macOS, a drag region spans the full width of the top of the window (see *visual-identity → macOS Hidden Inset Titlebar Layout*), and a press inside it enters window drag or zoom rather than reaching what is beneath. The header SHALL be positioned so that the change name lies **clear of that region**, so a click on the name copies it rather than starting a window drag, and a double-click does not toggle window zoom. That clearance SHALL hold at **every scroll position**, not only at scroll top. The header's own background SHALL continue to span the full pane width across the cleared area, so no document content is visible above the identity at any scroll position. The drag region SHALL be left intact: the area the header clears SHALL remain draggable, and no exception SHALL be carved out of it. This clearance is a property of the native window only; the served web UI renders no such region and SHALL receive no offset.

**Anchoring.** Because the header occupies the top of the pane's scroll port, scroll anchors (see *Outline Navigation* and *Fragment Links Resolve Within the Document* in the `document-outline` capability) SHALL account for its height: a heading scrolled to SHALL come to rest fully visible below the header, never underneath it. The height SHALL be taken from the rendered header rather than from a fixed constant, because the change name renders in full and therefore wraps at narrow pane widths — and because the macOS clearance changes that height. Any clearance SHALL therefore be inside the measured element.

**Placement.** The header SHALL be horizontally aligned with the artifact's prose column — sharing its width bound and horizontal origin — so it reads as heading the document rather than floating in the pane.

#### Scenario: Detail pane names the change it is rendering

- **WHEN** the user selects any artifact of a change in the tree
- **THEN** the detail pane displays that change's directory name above the rendered markdown
- **AND** the name is shown in full, with no truncation or ellipsis
- **AND** the name shown is the change's directory name, not its proposal title

#### Scenario: Branch appears as a chip beside the name

- **WHEN** the detail pane renders an artifact of a change in a git worktree on a named branch
- **THEN** the header shows that branch name as an outlined chip following the change name

#### Scenario: The branch chip carries the workspace's palette colour

- **WHEN** the detail pane renders an artifact of a change whose owning workspace has a configured palette colour
- **THEN** the branch chip's text and border render in a contrast-safe shade of that colour
- **AND** the colour is the one configured for the workspace owning the worktree the artifact was read from

#### Scenario: The branch chip stays neutral when no palette colour is configured

- **WHEN** the detail pane renders an artifact of a change whose owning workspace has no configured palette colour
- **THEN** the branch chip renders in the neutral ink it renders in today
- **AND** no colour is derived or substituted in place of the missing one

#### Scenario: The header chip and the tree chip agree

- **WHEN** the tree and the detail pane are both visible, and each shows a chip naming the same branch of the same change
- **THEN** the two chips render identically, in the same tint and treatment
- **AND** this holds for every palette colour and for a workspace with none configured

#### Scenario: A flat-workspace artifact shows no branch chip

- **WHEN** the detail pane renders an artifact of a change in a flat (non-git) workspace
- **THEN** the header shows the change's directory name alone
- **AND** no branch chip is rendered

#### Scenario: An archived change shows no branch chip

- **WHEN** the detail pane renders an artifact of an archived change
- **AND** the worktree path it was read from hosts active changes on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the header
- **AND** no palette colour is applied, there being no chip to tint

#### Scenario: The header reports when the artifact last changed

- **WHEN** the detail pane renders an artifact whose file was last written some interval ago
- **THEN** the header displays that interval in relative terms
- **AND** the interval is measured from the modification time of that artifact's own file

#### Scenario: A sibling artifact's edit is not reported as this one's

- **WHEN** the detail pane is rendering a change's `proposal.md`
- **AND** `tasks.md` in the same change directory is written
- **THEN** the interval reported for the `proposal.md` on screen is unchanged
- **AND** it continues to reflect when `proposal.md` itself was last written

#### Scenario: The label advances while the reader stays on the artifact

- **WHEN** the detail pane has rendered an artifact and enough time passes for the reported interval to change
- **AND** the user has neither navigated away nor taken any action
- **AND** nothing on disk has changed
- **THEN** the displayed interval advances to reflect the time now elapsed

#### Scenario: A rewrite with identical bytes still updates the label

- **WHEN** the detail pane is rendering an artifact
- **AND** that artifact's file is rewritten with content identical to what is displayed, moving its modification time
- **THEN** the reported interval updates to reflect the new modification time
- **AND** the rendered document and the reading position are unchanged

#### Scenario: The advancing label never moves the change name

- **WHEN** the reported interval advances, including across a change of unit
- **THEN** the label occupies the same width as before
- **AND** the change name occupies the same width and wraps at the same points
- **AND** no element of the header changes position

#### Scenario: A modification time in the future is not shown as future

- **WHEN** the detail pane renders an artifact whose file carries a modification time later than the present
- **THEN** the header presents the artifact as having changed at the present moment
- **AND** no future interval is displayed

#### Scenario: The header and the tree word an interval the same way

- **WHEN** the tree and the detail pane are both visible, each showing a relative time
- **THEN** an interval of the same length is rendered in the same words on both
- **AND** this holds at every tier of the vocabulary

#### Scenario: An artifact with no readable modification time shows no label

- **WHEN** the detail pane renders an artifact whose file reports no usable modification time
- **THEN** the artifact's markdown is displayed as normal
- **AND** no last-changed label is rendered
- **AND** no default, derived, or epoch time is shown in its place

#### Scenario: An archived artifact reports its modification time like any other

- **WHEN** the detail pane renders an artifact of an archived change
- **THEN** the header reports that file's modification time under the same rule as an active change's
- **AND** no alternative source, such as the archive date in the directory name, is substituted

#### Scenario: The label stops when the artifact it described is gone

- **WHEN** the detail pane's artifact target changes or clears while the label is advancing
- **THEN** the label ceases to advance for the artifact that is no longer rendered
- **AND** no update is applied to a header describing a different artifact

#### Scenario: One click copies the whole name

- **WHEN** the user clicks once on the change name in the header
- **THEN** exactly that change name is placed on the clipboard
- **AND** the name is also selected, as confirmation of what was copied
- **AND** a successful copy is indicated and announced

#### Scenario: The copied value excludes the branch

- **WHEN** the user clicks once on the change name of a change whose branch chip is displayed
- **THEN** the clipboard contains the change name only
- **AND** the branch chip's text is not part of what was copied, nor of the selection

#### Scenario: The copied value excludes the last-changed label

- **WHEN** the user clicks once on the change name while the last-changed label is displayed
- **THEN** the clipboard contains the change name only
- **AND** the label's text is not part of what was copied, nor of the selection

#### Scenario: Copy works where the asynchronous clipboard API is unavailable

- **WHEN** the web UI is reached over a non-loopback bind on a plain-HTTP origin, where `navigator.clipboard` is not exposed
- **AND** the user clicks once on the change name
- **THEN** the name is still placed on the clipboard, by the synchronous mechanism over the selection
- **AND** no failure is reported to the user

#### Scenario: A refused copy leaves the value selected

- **WHEN** a copy is attempted and the clipboard write is refused
- **THEN** the failure is distinguished from a success, not reported as one
- **AND** the change name remains selected, so the platform's copy shortcut completes the action

#### Scenario: Confirming a copy does not move the header

- **WHEN** a copy succeeds and the header shows its confirmation
- **THEN** the change name occupies the same width as before the copy
- **AND** no element of the header changes position

#### Scenario: Keyboard copies without a chord

- **WHEN** the user moves focus to the change name and presses Enter or Space
- **THEN** the same copy occurs as for a click
- **AND** the focus indicator is visible on the name
- **AND** the platform's own copy shortcut retains its native meaning

#### Scenario: Identity survives scrolling a long artifact

- **WHEN** the user scrolls an artifact long enough that its first line leaves the viewport
- **THEN** the change-identity header remains visible

#### Scenario: An anchored section is not obscured by the header

- **WHEN** the user selects a section or task row that scrolls the artifact to that anchor
- **THEN** the anchored section or task comes to rest fully visible
- **AND** it is not positioned underneath the change-identity header, at any header height

#### Scenario: The change name is clickable in the native macOS window

- **WHEN** the application runs in the native window on macOS, where the titlebar drag region covers the top of the window
- **AND** the user clicks once on the change name
- **THEN** the name is copied
- **AND** the window does not begin a drag
- **AND** a double-click on the name does not toggle window zoom

#### Scenario: Clearance holds while the artifact is scrolled

- **WHEN** the application runs in the native window on macOS and the artifact is scrolled to any position
- **THEN** the change name remains clear of the titlebar drag region and remains clickable
- **AND** no document content is visible above the header

#### Scenario: The titlebar drag region keeps working

- **WHEN** the application runs in the native window on macOS
- **AND** the user presses in the top 32px of the window over the detail pane, outside the change name
- **THEN** the window enters native drag mode exactly as it does elsewhere along the strip

#### Scenario: The served web UI takes no titlebar offset

- **WHEN** the web UI is served in a browser, which renders no titlebar drag region
- **THEN** the header takes no clearance offset
- **AND** the identity sits at the top of the pane

#### Scenario: Non-artifact targets are unaffected

- **WHEN** the detail pane renders the Dashboard, a commit's detail view, the workspace file browser, the Archive view, or the Settings view
- **THEN** no change-identity header is rendered over it
- **AND** each of those views keeps the header it renders today

### Requirement: Artifact Tab Strip in the Change Header

While the detail pane's target is an OpenSpec artifact, the change header (see *Change Identity Header in the Detail Pane*, *Rows*) SHALL render a **tab strip** offering the change's artifacts, in the fixed order Proposal, Design, Tasks, then one tab per capability spec in listing order. Only artifacts **present on disk** for the rendered instance SHALL be offered: an absent artifact has no tab, dimmed or otherwise, so the strip never offers a control that cannot render. Presence is taken from the same aggregation the tree reads (`ChangeData.artifacts`), and the strip SHALL re-derive when that aggregation refreshes, so a tab appears when its file appears. In the read-only form, presence is the on-demand status the `archive-browser` capability determines for the selected copy.

The **active tab** is the artifact the current address names. Activating a tab is a **navigation**: it SHALL form the artifact address for that artifact in the same instance and navigate to it, so history, deep links and reader windows treat it exactly as a tree click was treated (see *History Entry Discipline* in the `view-routing` capability — one entry per artifact shown). When the file behind the active tab vanishes, the tab SHALL remain active while the document view reports the document missing (see *A Vanished Document Is Reported, Not Followed* in `reader-window`); the strip SHALL NOT switch tabs on the reader's behalf.

The **Tasks tab** SHALL carry the change's task progress in its trailing slot: the task-progress meter (per *Task Progress Meter* in `visual-identity`) while at least one task is incomplete, the completion ✓ glyph when every task is complete, and neither when the change parses no tasks — the same rule the change row applies, fed by the same counts, so the two never disagree on one screen.

**Keyboard.** The strip SHALL be a WAI-ARIA tab list with **manual activation**: it occupies one position in the pane's Tab order, focus moves among tabs with ArrowLeft, ArrowRight, Home and End without changing what is shown, and Enter or Space activates the focused tab. Each tab exposes `role="tab"` with `aria-selected` reflecting whether it is active; the strip exposes `role="tablist"`. Automatic activation is deliberately not used, because activation navigates and a history entry per traversed tab would make Back useless.

#### Scenario: Only present artifacts are offered

- **WHEN** the detail pane renders an artifact of a change that has a proposal, tasks and two capability specs but no design
- **THEN** the strip offers Proposal, Tasks and the two specs, in that order
- **AND** no Design tab is rendered

#### Scenario: Activating a tab navigates

- **WHEN** the user activates the Tasks tab while the proposal is shown
- **THEN** the detail pane renders the change's tasks
- **AND** a subsequent back gesture returns to the proposal

#### Scenario: The Tasks tab carries progress

- **WHEN** the change has at least one incomplete task
- **THEN** the Tasks tab shows the task-progress meter
- **WHEN** every task is complete
- **THEN** the Tasks tab shows the completion ✓ and no meter
- **WHEN** the change parses no tasks
- **THEN** the Tasks tab shows neither

#### Scenario: A newly written artifact gains a tab

- **WHEN** the detail pane renders a change's proposal and a `design.md` is then written into the change directory
- **THEN** the strip offers a Design tab once the aggregation refreshes
- **AND** the proposal remains shown

#### Scenario: Arrow keys move focus without navigating

- **WHEN** the strip has focus on the Proposal tab and the user presses ArrowRight twice
- **THEN** focus rests on the third tab
- **AND** the proposal is still shown and no history entry was created
- **WHEN** the user then presses Enter
- **THEN** the detail pane navigates to the focused tab's artifact

#### Scenario: The read-only form derives tabs from the selected copy

- **WHEN** the archive reader renders an archived change whose selected copy has a proposal and one spec on disk
- **THEN** the strip offers Proposal and that spec only

### Requirement: Instance Switcher in the Change Header

When the change the detail pane is rendering has **more than one rendered instance** (see *Workspace Tree Hierarchy*), the change header SHALL render an **instance switcher** row between the identity row and the tab strip, offering one control per rendered instance. When the change has exactly one rendered instance, no switcher row SHALL be rendered and no space reserved for it.

Each instance's control SHALL be labelled by its worktree folder basename, followed by the instance's branch as an outlined chip tinted exactly as the identity row's chip is (see *Change Identity Header in the Detail Pane*, *Branch chip colour*) — the basename alone when the branch is not known — and by its divergence label when it has one (see *Per-Instance Divergence Label*). The control for the instance currently rendered SHALL be marked as selected and exposed as such to assistive technology.

Activating an instance's control is a **navigation** to the same artifact kind (and capability) in the chosen instance, with the address carrying the instance segment per *Shortest Unambiguous Address* in the `view-routing` capability. When the chosen instance does not hold that artifact, the navigation SHALL open the chosen instance's default artifact instead (see *Workspace Tree Hierarchy*), never a read error.

In the **read-only form** the switcher offers the archived change's **copies**, labelled as *Copy Selection Within an Opened Archived Change* in the `archive-browser` capability specifies, with no branch chip and no divergence label; it renders as a plain label when there is one copy, as that requirement already demands.

#### Scenario: A singleton change shows no switcher

- **WHEN** the detail pane renders an artifact of a change with exactly one rendered instance
- **THEN** the header contains no instance switcher row

#### Scenario: A multi-instance change names each instance

- **WHEN** the detail pane renders an artifact of a change hosted in two worktrees on different branches, one of which is diverged
- **THEN** the header shows two controls, each labelled by its worktree basename and branch chip
- **AND** the diverged instance's control carries the `[diverged]` label
- **AND** the rendered instance's control is marked selected

#### Scenario: Switching instance keeps the artifact

- **WHEN** the design of the main worktree's instance is shown and the user activates the feature worktree's control
- **THEN** the detail pane renders the feature worktree's design
- **AND** the address names that instance

#### Scenario: Switching to an instance lacking the artifact opens its default

- **WHEN** the design of one instance is shown and the user switches to an instance that has no `design.md`
- **THEN** the detail pane renders that instance's default artifact
- **AND** no read error is shown

#### Scenario: The read-only form offers copies without branches

- **WHEN** the archive reader renders an archived change with two copies
- **THEN** the switcher offers the two copies under the archive's copy labels
- **AND** no branch chip or divergence label is rendered
