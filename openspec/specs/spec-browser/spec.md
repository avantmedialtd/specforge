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

The tree pane SHALL display tracked workspaces grouped by repository where applicable. For each git repository with at least one tracked workspace, the tree SHALL render a top-level Repo group node containing the repository's logical changes. For each non-git workspace, the tree SHALL render a top-level workspace node containing that workspace's changes directly, as before.

A logical change groups every `ChangeInstance` that shares the same `(repository_id, change_directory_name)` tuple. Inside a Repo group, each logical change is rendered according to its instance count: a logical change with exactly one instance SHALL be rendered as a flat instance row with no parent disclosure; a logical change with two or more instances SHALL be rendered as a disclosure parent row with one child row per instance.

Each `ChangeInstance` row, when rendered, SHALL expose the same four artifact nodes — Proposal, Specs, Design, Tasks — in fixed order, mirroring the existing artifact subtree. The Specs node, when present, contains one child per capability spec file. The Tasks node, when present, contains one child per section in that instance's `tasks.md`, and each section contains one child per task line.

The tree SHALL render **active changes only**. Archived logical changes SHALL NOT appear anywhere in the tree — neither in an Active section nor in a separate Archive section. Archived changes are browsed exclusively in the dedicated Archive view (see the *Archive View* requirement in the `archive-browser` capability).

A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — and opens the workspace file browser for the row's workspace in the detail pane (see the `workspace-file-browser` capability). No placeholder child row SHALL be rendered beneath an empty top-level row. A top-level row whose only changes are archived SHALL therefore render as a leaf with a `0` active count, the same as a row with no changes at all.

#### Scenario: Git repo with multiple worktrees shown as one Repo group

- **WHEN** a repository has three tracked worktrees, two of which contain a change with the same directory name
- **THEN** the tree shows one top-level Repo group for that repository
- **AND** the two-instance change appears under a disclosure parent row with both instances as children
- **AND** any single-instance change appears as a flat row directly under the Repo group

#### Scenario: Non-git workspace shown as a standalone top-level node

- **WHEN** a tracked workspace is not inside a git repository
- **THEN** the workspace is rendered as a top-level node (not a Repo group)
- **AND** the workspace's changes are rendered directly underneath without instance aggregation

#### Scenario: Artifact subtree appears under each instance

- **WHEN** an instance row is expanded (or, for a singleton, the flattened row is expanded)
- **THEN** the four artifact nodes appear in the order: Proposal, Specs, Design, Tasks
- **AND** the contents of each artifact node are read from that instance's `worktree_path`

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
- **THEN** the row re-renders as a disclosure parent
- **AND** the count badge advances from `0` to the new count
- **AND** the disclosure's open/closed state is governed by the user's persisted override for that row, if any, and otherwise by the row's default-open behaviour

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

### Requirement: Section and Task Scroll Anchors

Clicking a section or individual-task node SHALL render `tasks.md` in the detail pane (if not already rendered) and scroll the detail pane to the corresponding heading or line.

#### Scenario: Click section scrolls to heading

- **WHEN** the user clicks a section node under a Tasks artifact
- **THEN** the detail pane shows the rendered `tasks.md` for that change
- **AND** the pane is scrolled so the section's heading is visible at the top of the pane

#### Scenario: Click task scrolls to task line

- **WHEN** the user clicks an individual task node under a section
- **THEN** the detail pane shows the rendered `tasks.md` for that change
- **AND** the pane is scrolled so the task's line is visible

### Requirement: Deferred Interaction Nodes

Clicking a logical-change parent disclosure row, a change node, or the Specs artifact node SHALL produce no observable effect in the detail pane. These node types are pure disclosure rows by design or are reserved for later UX work. Clicking a top-level workspace node or a Repo group node is no longer deferred: it opens the workspace file browser in the detail pane — see the *File Browser Surface* requirement in the `workspace-file-browser` capability.

#### Scenario: Click logical-change parent disclosure is a no-op

- **WHEN** the user clicks a logical-change parent disclosure row of a multi-instance change
- **THEN** the detail pane's current contents are unchanged
- **AND** the row's expand/collapse state toggles in response to the click on its disclosure caret

#### Scenario: Click change is a no-op

- **WHEN** the user clicks a change node under a non-git workspace
- **THEN** the detail pane's current contents are unchanged

#### Scenario: Click Specs artifact node is a no-op

- **WHEN** the user clicks the Specs artifact node of an instance or a change
- **THEN** the detail pane's current contents are unchanged

### Requirement: Reactive Updates from Filesystem

The tree pane and the detail pane SHALL reflect on-disk changes within the watcher's debounce window without requiring user action. After the watcher finishes processing a debounced batch of filesystem events, the *first* refresh the frontend performs in response to that batch SHALL observe the post-batch state — the UI MUST NOT lag behind by one event for any on-disk change, including content-only changes inside a change directory that is already tracked (artifact file creation, task checkbox toggles, edits to spec or proposal markdown).

The detail pane's refresh SHALL re-read the artifact it is currently rendering. It SHALL be driven by the change notification alone and MUST NOT be conditioned on the workspace named in that notification's payload, because a notification MAY carry any tracked workspace as a carrier rather than the workspace whose contents changed.

A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT alter the rendered document when the artifact's bytes are unchanged. (This narrows a previous contract, under which such a refresh was required to be wholly unobservable when the bytes were unchanged. That is no longer correct: a file rewritten with identical bytes has a **new modification time**, and the header reports modification time — see *Change Identity Header in the Detail Pane*, "Last changed". The protection this clause exists to give — an undisturbed reader, no loading indicator, no repaint of the document — is unchanged; what narrows is its scope, from the whole pane to the document within it.) When such a refresh fails to read the artifact, the pane SHALL continue to display the content it already holds rather than replacing it with an error. A read the user initiated by selecting an artifact retains its existing loading and error presentation.

The cost of the unfiltered subscription SHALL remain bounded by this guarantee: a refresh that changes neither the artifact's bytes nor its modification time SHALL do no work beyond the read itself, and one that changes only the modification time SHALL NOT re-render the document.

#### Scenario: Tree updates when new change appears

- **WHEN** a new change directory is created on disk in a registered workspace
- **THEN** the new change appears as a child of that workspace in the tree

#### Scenario: Detail pane updates when shown file is edited

- **WHEN** the detail pane is currently rendering an artifact's markdown
- **AND** that markdown file is modified on disk
- **THEN** the detail pane re-renders with the updated content

#### Scenario: Reading position survives a refresh the user did not initiate

- **WHEN** the detail pane is rendering an artifact and the user has scrolled away from the top
- **AND** that artifact's file is modified on disk while the user's selection is unchanged
- **THEN** the pane renders the updated content
- **AND** the reading position is preserved — the pane neither scrolls to the top nor scrolls back to a section or task the user selected in the tree earlier
- **AND** no loading indicator is presented

#### Scenario: Refresh with unchanged content is not observable

- **WHEN** the detail pane is rendering an artifact
- **AND** a filesystem change elsewhere triggers a refresh whose read returns content identical to what is displayed, with an unchanged modification time
- **THEN** the rendered output, the reading position, and the loading indicator are all unchanged

#### Scenario: A modification-time-only change updates the header and nothing else

- **WHEN** the detail pane is rendering an artifact
- **AND** a refresh returns content identical to what is displayed but a newer modification time
- **THEN** the header's last-changed label updates
- **AND** the rendered document is not re-rendered
- **AND** the reading position is preserved and no loading indicator is presented

#### Scenario: Refresh is not conditioned on the workspace the notification names

- **WHEN** the detail pane is rendering an artifact belonging to one tracked workspace
- **AND** a filesystem-change notification arrives naming a different tracked workspace
- **THEN** the pane still re-reads its artifact and renders the current on-disk content

#### Scenario: Failed background read preserves the displayed content

- **WHEN** the detail pane is rendering an artifact
- **AND** a refresh the user did not initiate fails to read that artifact, because its file was removed, became unreadable, or was caught mid-write
- **THEN** the pane continues to display the content it already loaded
- **AND** no error state replaces it

#### Scenario: Failed selection read still reports the error

- **WHEN** the user selects an artifact whose file cannot be read
- **THEN** the detail pane presents its error state

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

### Requirement: Active-Instance Indicator

For every logical change with at least two instances, the application SHALL identify the *primary instance* as the one with the most recent modification time across the files of its change directory, and render a visible active indicator (●) on that instance's row. Singleton logical changes (one instance) SHALL NOT display the active indicator — there is nothing to disambiguate.

#### Scenario: Most-recently-modified instance carries the active dot

- **WHEN** a logical change has two instances and one has been modified more recently than the other
- **THEN** the more recently modified instance's row displays the ● indicator
- **AND** the other instance's row does not display the indicator

#### Scenario: Indicator moves when activity moves

- **WHEN** the secondary instance of a logical change is modified, making it the more recently modified one
- **THEN** the ● indicator moves to that instance's row within the watcher debounce window

### Requirement: Per-Instance Divergence Label

For every `ChangeInstance` that is not on the repository's default branch, the application SHALL compute and display at most one divergence label by comparing the instance's change directory contents against the default-branch instance of the same logical change. The labels are:

- `[diverged]` — the change exists in both the default-branch instance and the non-default instance, but the file contents differ at the byte level.
- `[stale]` — the change is archived on the default branch (under `openspec/changes/archive/`) but is still active in the non-default instance.

If the change does not exist on the default branch at all, or if no default branch is known, or if the contents are identical, the instance SHALL display no divergence label.

#### Scenario: Diverged content gets the diverged label

- **WHEN** an instance on a non-default branch has different content under `openspec/changes/<name>/` than the default-branch instance of the same logical change
- **THEN** the instance row displays the `[diverged]` label

#### Scenario: Stale-vs-archive gets the stale label

- **WHEN** the default-branch instance of a logical change is in `openspec/changes/archive/<name>/`
- **AND** a non-default instance of the same logical change is in `openspec/changes/<name>/` (still active)
- **THEN** the non-default instance row displays the `[stale]` label

#### Scenario: Branch-only change gets no label

- **WHEN** a logical change has no instance on the default branch (it was created only on a feature branch)
- **THEN** every non-default instance displays no divergence label

#### Scenario: Identical content gets no label

- **WHEN** a non-default instance has byte-identical content to the default-branch instance of the same logical change
- **THEN** the non-default instance row displays no divergence label

#### Scenario: No default branch produces no labels

- **WHEN** the repository has no detected default branch
- **THEN** no instance of any logical change in that repository displays a divergence label

### Requirement: Singleton Logical-Change Flattening and Promotion

A logical change with exactly one instance SHALL be rendered as a single flat row directly under its Repo group (or under the Active / Archive section as appropriate). When the instance count grows to two or more — for example because a new worktree begins working on the same change — the row SHALL be promoted to a disclosure parent with one child row per instance. When the instance count drops back to one, the row SHALL collapse back to a flat row.

#### Scenario: Singleton renders without a disclosure parent

- **WHEN** a logical change has exactly one instance
- **THEN** the tree shows a single row for that instance directly under its Repo group
- **AND** no separate parent disclosure row is rendered

#### Scenario: Promotion when a second instance appears

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the row is replaced with a disclosure parent row that, when expanded, shows both instances
- **AND** the parent is expanded by default so the user sees both new and previously-visible instances without an extra click

#### Scenario: Collapse when count drops back to one

- **WHEN** a previously multi-instance logical change loses every instance but one
- **THEN** the disclosure parent disappears and the remaining instance is rendered as a flat row

### Requirement: Instance Row Chrome

A **multi-instance child row** — a `ChangeInstance` rendered beneath a multi-instance logical-change disclosure parent — SHALL display the instance's branch name as its primary label, falling back to the worktree path's basename when the branch is not known (detached HEAD, bare worktree). Because the disclosure parent already names the change, the branch alone distinguishes the child, and the child SHALL remain a single line. The child row SHALL additionally display, in its trailing meta slot, a task-progress meter (see the *Task Progress Meter* requirement in the `visual-identity` capability) and the relative modification time. The divergence label (when present) and the active indicator (when present) attach to the child row alongside these elements.

A **flattened singleton instance row** (a logical change with exactly one instance, per *Singleton Logical-Change Flattening and Promotion*) and a **flat-workspace change row** are NOT governed by this requirement. They render per *Two-Line Sole-Change-Row Layout*, which gives the change's own label the primary line and relocates the branch, worktree folder, and status meta to a second line. In particular, branch-as-primary-label is a multi-instance-child behaviour only; a singleton's primary label is the change's own name, never its branch.

#### Scenario: Multi-instance child label uses branch when available

- **WHEN** a multi-instance logical change's child instance is on a named branch
- **THEN** the child row's primary label is the branch name
- **AND** the child row remains a single line

#### Scenario: Multi-instance child label falls back to path basename

- **WHEN** a multi-instance child instance's worktree is not on a named branch (detached HEAD or no git context)
- **THEN** the child row's primary label is the basename of the worktree path

#### Scenario: Progress and modification time are shown on the child row

- **WHEN** a multi-instance child row is rendered for a change with at least one incomplete task
- **THEN** the child row shows the instance's task progress as a fill meter (an outlined track with a fill whose width is `completedTasks / totalTasks`), with **no** inline digits
- **AND** the exact count is available via the meter's `title` tooltip ("N of M tasks") and its `role="progressbar"` aria attributes
- **AND** the child row shows a relative modification time (e.g. `12m ago`) in its trailing meta slot

### Requirement: Default Expansion of Tree Nodes

Every collapsible node in the workspace tree SHALL be rendered with a default expansion state derived from the node's current data at render time, with no prior interaction required.

For most node types — Repo groups, flat workspaces, multi-instance logical-change parents, instance rows, the Proposal/Design/Specs artifact nodes, capability rows under Specs, and individual task rows — the default SHALL be "expanded".

The **Tasks artifact node** SHALL default to "collapsed" whenever it is collapsible (its change has at least one section), regardless of task-completion state. This keeps a change's task rows out of the tree until the user opts into them by expanding the Tasks node; the node's task-progress meter or completion ✓ remains visible in its meta slot while collapsed (see *Tasks Artifact Node Progress*).

For one further node type the default depends on completion state:

- A **Section node** SHALL default to "collapsed" when its section has at least one task and every task in it is complete; otherwise it SHALL default to "expanded".

The user MAY override a node's default in either direction by clicking its disclosure caret. The application records overrides in two independent sets:

- A `collapsed` set of node IDs the user has closed against a default-open node.
- An `expanded` set of node IDs the user has opened against a default-closed node.

A node's rendered open/closed state SHALL be computed as follows:

- For a node whose current default is "open": open iff its ID is **not** in the `collapsed` set.
- For a node whose current default is "closed": open iff its ID **is** in the `expanded` set.

Because the Tasks artifact node's default is now "closed" unconditionally, a user who opens it has the node's ID recorded in the `expanded` set, and that preference persists across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: First-ever launch shows the tree expanded except for Tasks nodes and completed Sections

- **WHEN** the user launches the application for the first time after this change ships
- **AND** at least one workspace has been registered
- **THEN** every collapsible row whose computed default is "expanded" is rendered open
- **AND** every collapsible Tasks artifact node is rendered collapsed, regardless of task-completion state
- **AND** every Section node whose section has at least one task and all of them complete is rendered collapsed
- **AND** every section with at least one incomplete task carries the "expanded" default, so its task rows are visible once its Tasks node is expanded

#### Scenario: New change appears with its Tasks node collapsed

- **WHEN** the workspace tree is already rendered
- **AND** a new change directory is added to a registered workspace on disk, triggering a `change-added` event
- **THEN** the new change's row appears in the tree expanded
- **AND** the change's Proposal, Specs, and Design artifact rows are rendered expanded
- **AND** the change's Tasks artifact row is rendered collapsed
- **AND** each Section under Tasks carries the per-node-type Section default (collapsed iff all of its tasks are complete) for when the Tasks node is expanded

#### Scenario: Promoted multi-instance parent appears expanded

- **WHEN** a previously-singleton logical change gains a second instance
- **THEN** the new disclosure parent row is rendered expanded by default
- **AND** the two instance rows beneath it are visible without any user click

#### Scenario: User expand overrides the collapsed Tasks node

- **WHEN** a Tasks artifact node is rendered collapsed by default
- **AND** the user clicks the Tasks row's disclosure caret
- **THEN** the Tasks node is re-rendered expanded
- **AND** its Section rows are visible, each per the Section default rule

#### Scenario: User expand overrides an auto-collapsed Section

- **WHEN** a Section node is rendered collapsed because every task in it is complete
- **AND** the user clicks the Section row's disclosure caret
- **THEN** the Section is re-rendered expanded
- **AND** its task rows are visible

#### Scenario: User collapse overrides a default-open node

- **WHEN** an in-progress Section node is rendered expanded
- **AND** the user clicks its disclosure caret
- **THEN** the Section is re-rendered collapsed
- **AND** its row continues to display its title (no other rows beneath it change state)

### Requirement: User Collapse State Persists Across Sessions

When the user clicks the disclosure caret of any tree node, the application SHALL persist the resulting override so that, after quitting and relaunching, the node is rendered in the same state without further user action.

The persisted state SHALL consist of two independent sets of node IDs:

- `collapsedTreeNodeIds` — node IDs the user has closed against a default-open node.
- `expandedTreeNodeIds` — node IDs the user has opened against a default-closed node.

A node's rendered state on launch SHALL be computed by combining its current default (derived from the node's data) with the matching override set, as defined in the *Default Expansion of Tree Nodes* requirement.

A persisted ID in one set whose node's default has since flipped to the other polarity SHALL be ignored — it is consulted only when the node's default again matches that set's role. The application is not required to garbage-collect such inert entries.

#### Scenario: Collapsed default-open node stays collapsed after restart

- **WHEN** the user collapses a default-open node (e.g., the Proposal artifact row of some change, or an in-progress Section)
- **AND** the user quits and relaunches the application
- **THEN** the same node is rendered in its collapsed state in the restored tree
- **AND** sibling nodes the user did not collapse are rendered per their own defaults

#### Scenario: Re-expanded default-open node stays expanded after restart

- **WHEN** the user has previously collapsed and persisted a default-open node
- **AND** the user re-expands that node in the current session
- **AND** the user quits and relaunches the application
- **THEN** the node is rendered in its expanded state in the restored tree

#### Scenario: Expanded default-closed node stays expanded after restart

- **WHEN** a Section node is collapsed by default because every task in it is complete
- **AND** the user clicks its disclosure caret to expand it
- **AND** the user quits and relaunches the application
- **THEN** the same Section is rendered in its expanded state in the restored tree
- **AND** other completed Sections the user did not expand remain collapsed

#### Scenario: Re-collapsed default-closed node stays collapsed after restart

- **WHEN** the user has previously expanded and persisted a default-closed Section
- **AND** the user re-collapses that Section in the current session
- **AND** the user quits and relaunches the application
- **THEN** the Section is rendered in its collapsed state in the restored tree

#### Scenario: Settings file with no expanded-IDs field loads cleanly

- **WHEN** the user launches a version of the application that supports the expanded-overrides set for the first time
- **AND** the existing settings file on disk was written by a previous version that has no `expandedTreeNodeIds` field
- **THEN** the application loads the settings file successfully
- **AND** the existing `collapsedTreeNodeIds` field is honoured as before
- **AND** the tree is rendered with an empty `expanded` override set, so every default-closed node renders collapsed until the user expands it

#### Scenario: Persistence write is bounded by user toggles

- **WHEN** the user toggles the same node open and closed several times in rapid succession
- **THEN** the persisted state eventually reflects the final toggled position for both sets
- **AND** the application does not write a settings file for every intermediate state (writes to each set are coalesced)

#### Scenario: Stale expand-override survives default flip without surfacing

- **WHEN** the user has expanded a completed Section (its ID is in the `expanded` set)
- **AND** a new incomplete task is added to that Section, flipping its default to "open"
- **THEN** the Section is rendered open (because the default-open path consults the `collapsed` set, which does not contain the ID)
- **AND** when the Section later returns to fully-complete, the persisted ID in the `expanded` set causes the Section to render expanded again (matching the user's earlier preference)

### Requirement: Tree Expansion Has No First-Sight Auto-Expansion Effect

The application SHALL NOT maintain a separate "first time we see this node, mark it expanded (or collapsed)" code path. A node's default state is derived from its current data on every render — not from any one-shot seeding effect that runs on view changes — and the user's override (if any) is applied on top of that default.

Revealing the node named by an address is not such a code path and is permitted, precisely because it is transient: it SHALL be applied above the override sets without writing them, and SHALL NOT trigger a settings write — see the *Navigation Reveal Is Transient* requirement in the `view-routing` capability. Following a link therefore never rewrites the recipient's stored tree preferences.

#### Scenario: No second-mount re-seeding

- **WHEN** the watcher emits a `cache-updated` event that causes the tree's `views` prop to re-render
- **THEN** the application does not run any effect that mutates the `collapsed` or `expanded` override sets in response to the new view

#### Scenario: User override survives a tree re-render

- **WHEN** the user has expanded an auto-collapsed Section or collapsed a default-open Section
- **AND** the watcher subsequently emits a `cache-updated` event for that workspace
- **THEN** the user's override is preserved after the re-render
- **AND** the application does not flip the node's state as a side effect of the view change

#### Scenario: A navigation reveal leaves the override sets untouched

- **WHEN** the user follows an address naming an artifact whose ancestor nodes they had previously collapsed
- **THEN** those ancestors are shown open so the addressed node is visible
- **AND** the `collapsed` and `expanded` override sets are unchanged
- **AND** no settings write is performed as a result

### Requirement: Auto-Collapse of Completed Task Groups

The workspace tree SHALL auto-collapse **Section nodes** when their work is complete, so the user's attention is drawn to in-progress work. The Tasks artifact node is collapsed by default unconditionally (see *Default Expansion of Tree Nodes*) and therefore does not participate in this completion-based rule.

- A **Section node** is considered complete when its section has at least one task (`tasks.length > 0`) and every task in it is complete.

When a Section is complete, its default expansion state SHALL be "collapsed". When it is not complete (or has no tasks at all), its default expansion state SHALL be "expanded". The default is recomputed on every render from the node's current data, so transitions between in-progress and complete take effect within the watcher's debounce window with no extra user action.

The completion-based auto-collapse rule SHALL apply only to Section nodes. Change rows, Instance rows, Repo groups, flat workspaces, multi-instance logical-change parents, the Proposal/Specs/Design artifact rows, capability rows under Specs, and individual task rows SHALL continue to default to "expanded" regardless of completion state. The Tasks artifact node SHALL default to "collapsed" regardless of completion state.

A user override (a click on the disclosure caret) SHALL take precedence over the default and SHALL persist across restarts per the *User Collapse State Persists Across Sessions* requirement.

#### Scenario: Tasks artifact node defaults collapsed regardless of completion

- **WHEN** a Tasks artifact node is collapsible (its change has at least one section)
- **AND** the user has not explicitly expanded it
- **THEN** the Tasks artifact node is rendered collapsed whether the change's tasks are all complete, partially complete, or all incomplete
- **AND** its meta slot shows the task-progress meter (in progress) or the trailing `✓` (complete) per *Tasks Artifact Node Progress*

#### Scenario: Tasks node with sections but no parseable tasks still defaults collapsed

- **WHEN** a change's `tasks.md` has at least one section heading but no parseable task lines (`totalTasks === 0`)
- **THEN** the Tasks artifact node is collapsible and is rendered collapsed by default
- **AND** its meta slot shows neither a progress meter nor a `✓` glyph

#### Scenario: Section collapses when all its tasks complete

- **WHEN** a Section has at least one task and every task in it is complete
- **AND** the user has not explicitly expanded that Section since it became complete
- **THEN** the Section node is rendered collapsed

#### Scenario: Section stays expanded when partially complete

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section node is rendered expanded by default

#### Scenario: Section with no tasks is unaffected by the auto-collapse rule

- **WHEN** a Section has zero tasks
- **THEN** the Section row is rendered as a leaf (no chevron), as it is today
- **AND** the auto-collapse rule does not apply

#### Scenario: Completing the last task in a change does not change its Tasks node expansion

- **WHEN** a Tasks artifact node is rendered collapsed by default
- **AND** an external edit to `tasks.md` marks the change's last incomplete task complete
- **AND** the watcher emits the update
- **THEN** the Tasks artifact node remains collapsed (it was already collapsed by default)
- **AND** its meta slot swaps the progress meter for the trailing `✓` within the watcher debounce window

#### Scenario: Completing the last task in a Section auto-collapses it

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an external edit to `tasks.md` marks the last incomplete task complete
- **AND** the watcher emits the update
- **THEN** the Section node is re-rendered collapsed within the watcher debounce window
- **AND** the user does not need to take any action

#### Scenario: Adding an incomplete task to a complete Section re-expands it

- **WHEN** a Section node is rendered collapsed because every task in it is complete
- **AND** an external edit to `tasks.md` adds a new incomplete task to that section
- **AND** the watcher emits the update
- **THEN** the Section node is re-rendered expanded within the watcher debounce window

#### Scenario: User can expand a collapsed Tasks node or auto-collapsed Section

- **WHEN** a Tasks artifact node (collapsed by default) or a Section node (collapsed because its tasks are all complete) is rendered collapsed
- **AND** the user clicks the row's disclosure caret
- **THEN** the node is re-rendered expanded
- **AND** the expansion persists across restarts

### Requirement: Completed Section Row Shows a Completion Glyph

Every Section row whose section has at least one task and whose every task is complete SHALL display a ✓ glyph in the row's meta column, regardless of the row's current expansion state.

This glyph distinguishes a Section that is collapsed because all its tasks are done from a Section the user has manually collapsed while work is still in progress. It mirrors the trailing ✓ glyph rendered in the Change-row meta cluster when every task in a change is complete (see *Change-Row Completion Glyph*) and the trailing ✓ rendered on the Tasks artifact node at completion (see *Tasks Artifact Node Progress*).

#### Scenario: Completed Section shows the glyph while collapsed

- **WHEN** a Section is rendered collapsed because every task in it is complete
- **THEN** the Section row displays a ✓ glyph in its meta column

#### Scenario: Completed Section shows the glyph while expanded

- **WHEN** a Section is rendered expanded (either by default because it has incomplete tasks, or because the user explicitly expanded an auto-collapsed Section)
- **AND** every task in the Section is in fact complete
- **THEN** the Section row displays the ✓ glyph in its meta column

#### Scenario: In-progress Section shows no glyph

- **WHEN** a Section has at least one incomplete task
- **THEN** the Section row does not display the ✓ glyph

#### Scenario: Empty Section shows no glyph

- **WHEN** a Section has zero tasks
- **THEN** the Section row does not display the ✓ glyph

### Requirement: Artifact Row Presence Treatment

For each artifact node (Proposal, Specs, Design, Tasks) rendered under an instance row or a flat-change row, the row's *leading slot* (the position to the immediate right of the chevron/spacer) SHALL be reserved for identity affordances only — the row SHALL NOT render an icon whose sole semantics are "the underlying artifact file is present on disk." When the artifact's underlying file is present, the row SHALL display only the chevron (or chevron-spacer), the row label, and any trailing meta the schema defines.

When the artifact's underlying file is absent, the row SHALL:

- render at `opacity: 0.45` of the row's normal appearance,
- set `pointer-events: none` (or otherwise be inert to mouse interaction) so that clicking the row produces no selection, no detail-pane change, and no hover styling,
- preserve its layout footprint (chevron-spacer, label, depth indent) so the four-row artifact block does not collapse,
- continue to be visible in the tree as a slot indicator for the missing artifact.

The Specs artifact node SHALL count as "present" iff at least one capability spec file is parsed under the change; otherwise it SHALL be treated as absent and dimmed per the rule above.

#### Scenario: Present artifact rows carry no leading existence icon

- **WHEN** an artifact node is rendered for an artifact whose underlying file is present
- **THEN** the row displays no leading existence-marker glyph (no `Check`, no `DotOutline`, no equivalent)
- **AND** the row renders at full opacity
- **AND** the row participates normally in click, hover, and selection

#### Scenario: Missing artifact rows are dimmed and non-interactive

- **WHEN** an artifact node is rendered for an artifact whose underlying file is absent
- **THEN** the row renders at `opacity: 0.45`
- **AND** the row does not respond to clicks (no selection, no detail-pane change)
- **AND** the row does not display a hover background
- **AND** the row still occupies its full layout slot (label visible, depth indent preserved) so the four-artifact block remains intact

#### Scenario: Specs artifact dimming follows capability-spec presence

- **WHEN** a change has no parsed capability spec files
- **THEN** the Specs artifact row is treated as absent and rendered dim + non-interactive
- **AND** when at least one capability spec file is parsed, the Specs row renders normally

### Requirement: Change-Row Completion Glyph

For change-aggregating rows that surface task progress — specifically the flat-workspace change row (`FlatChangeNode`) and the per-instance row (`InstanceNode`) — when every parsed task in the change is complete (`totalTasks > 0` and `completedTasks === totalTasks`), the row SHALL render a trailing `Check` glyph in the row's meta cluster. On the per-instance row, the in-progress task-progress meter is hidden at 100% (see *Instance Row Chrome* and the *Task Progress Meter* requirement in `visual-identity`) and the `Check` occupies the meta position the meter would otherwise hold. When at least one task is incomplete, or when the change has no tasks at all, the row SHALL NOT render the trailing `Check` glyph.

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

### Requirement: Leaf-Task Completion Rendering

The tree pane SHALL render each leaf-task row using only its label text, with no leading completion glyph in either the completed or the pending state. A completed task (a `- [x]` line) SHALL render its label with a line-through text decoration AND the faint/dimmed task text colour. A pending task (a `- [ ]` line) SHALL render its label with no text decoration in the default task-label colour. The completion state of a leaf task SHALL be conveyed by this text treatment alone, and SHALL NOT be conveyed by a leading checkbox or checkmark glyph.

This requirement governs leaf-task rows only. It SHALL NOT alter the aggregate completion indicators defined elsewhere in this capability: the trailing `✓` completion glyph on a fully-complete Section, flat-Change, and per-Instance row, and the task-progress meter (see *Task Progress Meter* in `visual-identity`), all remain unchanged.

#### Scenario: Completed leaf task renders struck-through and dimmed

- **WHEN** a Section node is expanded and one of its task lines is `- [x]`
- **THEN** that task's row renders its label with a line-through text decoration and the dimmed task text colour
- **AND** no leading checkbox or checkmark glyph is rendered on the row

#### Scenario: Pending leaf task renders plain

- **WHEN** a Section node is expanded and one of its task lines is `- [ ]`
- **THEN** that task's row renders its label with no text decoration in the default task-label colour
- **AND** no leading checkbox or checkmark glyph is rendered on the row

#### Scenario: Aggregate completion indicators are retained

- **WHEN** every task in a Section is complete (and likewise for a fully-complete flat-Change row or per-Instance row)
- **THEN** the Section / flat-Change / Instance row continues to render its trailing `✓` completion glyph as before
- **AND** the task-progress meter continues to depict progress unchanged (per the *Task Progress Meter* requirement)

#### Scenario: Selection composes with the strikethrough treatment

- **WHEN** a completed leaf-task row is the currently selected node
- **THEN** the row shows the standard selection treatment
- **AND** the row's label remains struck-through and rendered in the dimmed task text colour

### Requirement: Tasks Artifact Node Progress

The Tasks artifact node under a change SHALL render its label as the plain text `Tasks`, with no parenthetical `(completed/total)` count appended. The node's completion progress SHALL instead be surfaced in the node row's trailing meta slot:

- While the change has at least one task and not every task is complete (`totalTasks > 0` and `completedTasks < totalTasks`), the node SHALL render a task-progress meter (see the *Task Progress Meter* requirement in the `visual-identity` capability) in its meta slot.
- When every task is complete (`totalTasks > 0` and `completedTasks === totalTasks`), the node SHALL render a trailing `✓` glyph in its meta slot in place of the meter, mirroring the Change-row completion glyph.
- When the change has no parseable tasks (`totalTasks === 0`), the node SHALL render neither a meter nor a `✓` glyph in its meta slot.

This requirement does not alter the Tasks node's auto-collapse default (see *Auto-Collapse of Completed Task Groups*); it changes only the node's label and the contents of its meta slot.

#### Scenario: In-progress Tasks node shows a meter

- **WHEN** the Tasks artifact node is rendered for a change with at least one incomplete task
- **THEN** the node's label is the plain text `Tasks` (no `(n/n)` suffix)
- **AND** the node's meta slot shows a task-progress meter whose fill width is `completedTasks / totalTasks`
- **AND** the exact count is available via the meter's `title` tooltip and aria attributes

#### Scenario: Completed Tasks node shows a check instead of the meter

- **WHEN** the Tasks artifact node is rendered for a change in which every task is complete
- **THEN** the node's meta slot shows a trailing `✓` glyph and no meter

#### Scenario: Tasks node with no parseable tasks shows neither meter nor check

- **WHEN** the Tasks artifact node is rendered for a change whose `tasks.md` parses zero tasks (`totalTasks === 0`)
- **THEN** the node's label is the plain text `Tasks`
- **AND** the node's meta slot contains neither a meter nor a `✓` glyph

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

A change row that is the **sole row for its change** SHALL render across two stacked lines within a single selectable row. Exactly two row types are sole change rows:

- a **flattened singleton instance row** — a git logical change with exactly one instance, rendered flat (no disclosure parent) per *Singleton Logical-Change Flattening and Promotion*; and
- a **flat-workspace change row** — a `ChangeData` row rendered directly under a non-git workspace node.

Multi-instance child rows (governed by *Instance Row Chrome*), multi-instance logical-change disclosure parents, Repo-group and workspace header rows, the Proposal/Specs/Design/Tasks artifact rows, capability rows, Section rows, and task rows are all excluded and SHALL remain single-line.

**Line 1 (primary).** Line 1 SHALL display the change's `proposal.md` title when one is extractable (see *Proposal Title Extraction*) — falling back, for a git singleton, to the logical change name, and for a flat-workspace change row, to its directory name. When a git singleton's line 1 shows the proposal title, the row SHALL expose the logical change name via its hover tooltip so the directory identity stays recoverable. The label SHALL render with slightly heavier weight than its artifact-row siblings so it reads as the row's heading, and SHALL own the full row width — no trailing branch chip or status meta shares the line — except for the favorite toggle's reserved trailing slot (see *Change-Row Favorite Toggle*); it SHALL ellipsize against that slot when it exceeds the available width. Line 1 carries no worktree identity, swatch, or colour tint on its text.

**Line 2 (detail).** Line 2 SHALL render at the tree's dense meta type tier, visually subordinate, and SHALL be indented to begin at line 1's text origin (past the chevron) so it reads as belonging to the row above it. Line 2 SHALL place worktree identity on its leading edge and status on its trailing edge:

- **Leading edge.** For a git singleton row the leading edge SHALL show the instance's branch name as an outlined chip (per *visual-identity → Outlined Chip Badges*) tinted to the owning workspace's palette colour — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour. When the branch is not known (detached HEAD, bare worktree), the chip SHALL show the worktree folder basename instead. A flat-workspace change row has no git worktree identity; in its place the leading edge SHALL show the change's identifier (`changeId`), the same identifier the row shows today.
- **Status (trailing).** Line 2 SHALL carry the row's existing status elements, with their existing presence rules, on its trailing edge. For a **git singleton row** these are the task-progress meter while work is in progress or the completion ✓ when every task is complete (per *Change-Row Completion Glyph* and *Tasks Artifact Node Progress*), the relative modification time, and the divergence label when present (per *Per-Instance Divergence Label*). For a **flat-workspace change row** the only status element is the completion ✓ when every task is complete; a flat-workspace row carries no progress meter, modification time, or divergence label. The active-instance indicator is a multi-instance-child element and SHALL NOT appear on a sole change row.

**Workspace-colour rail.** A sole change row SHALL tint its inline-start border — the 2px slot the selection bar occupies — with the owning workspace's palette colour, so each change reads as belonging to its workspace and the colour ties the row to its branch chip top-to-bottom. While the row is selected the selection bar (the 2px `--accent` border, per *visual-identity → Tree Row Selection Model*) SHALL take precedence and replace the rail; the rail SHALL reappear when the row is deselected. A workspace with no configured palette colour renders no rail. Header rows and the other excluded row types do not render the rail.

**One interaction unit.** The two lines SHALL form a single interaction unit: one click target that selects the change and one selection unit. The selection treatment (the 2px `--accent` inline-start bar plus its tint wash) and the hover wash SHALL span both lines. The disclosure chevron SHALL toggle the row's artifact subtree exactly as it does today and SHALL remain associated with the row as a whole. The favorite toggle (see *Change-Row Favorite Toggle*) is the row's only other nested control; like the chevron, activating it SHALL NOT select the change.

#### Scenario: Git singleton renders its proposal title on the first line

- **WHEN** a git logical change has exactly one instance and its `proposal.md` yields a title
- **THEN** line 1 shows that title across the full row width, in a slightly heavier weight than the artifact rows below it
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

#### Scenario: Multi-instance child row is excluded and stays single-line

- **WHEN** a logical change has two or more instances and is rendered as a disclosure parent with child rows
- **THEN** each child row remains a single line per *Instance Row Chrome*
- **AND** no child row adopts the two-line layout

#### Scenario: Completed sole change row shows its completion glyph on the detail line

- **WHEN** a sole change row's change has at least one task and every task is complete
- **THEN** line 2's trailing edge shows the completion ✓ in place of the progress meter

#### Scenario: Selection and hover span both lines of a sole change row

- **WHEN** a sole change row is selected, or the pointer hovers over either of its two lines
- **THEN** the selection bar and tint (or the hover wash) cover both lines as one contiguous row
- **AND** a click anywhere on either line — outside the disclosure chevron and the favorite toggle — selects the change and updates the detail pane

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

The workspace tree SHALL be fully operable from the keyboard as a WAI-ARIA tree with a roving tabindex: the tree occupies exactly one position in the window's Tab order, and within it a single current row carries focus, movable with the keyboard. Keyboard activation SHALL reuse the same selection contract as pointer clicks — a row whose click renders content in the detail pane renders the same content when activated by keyboard, and rows whose clicks are disclosure-only remain disclosure-only.

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

- **WHEN** the user presses ArrowRight on a collapsed expandable row
- **THEN** the row expands, honouring the same expansion-persistence behavior as a chevron click
- **WHEN** the user presses ArrowRight on an already-expanded row
- **THEN** focus moves to the row's first child
- **WHEN** the user presses ArrowLeft on an expanded row
- **THEN** the row collapses
- **WHEN** the user presses ArrowLeft on a collapsed or leaf row that has a parent row
- **THEN** focus moves to the parent row

#### Scenario: Enter and Space activate the current row

- **WHEN** the user presses Enter or Space on a row whose pointer click renders content in the detail pane (instance, proposal/design/tasks artifact, capability-spec, section, and task rows)
- **THEN** the row is selected and the detail pane renders exactly what a pointer click on that row would render
- **WHEN** the user presses Enter or Space on a disclosure-only grouping row (workspace, repo, logical change, and change rows, plus the Specs artifact row — whose pointer click also renders no content)
- **THEN** the row's expansion toggles, identically to a chevron click

#### Scenario: Debounced follow-focus opens content without per-keystroke reads

- **WHEN** keyboard focus comes to rest on a row whose pointer click renders content in the detail pane, and remains there for a short settle delay (approximately 150 ms)
- **THEN** the detail pane renders that row's content as if the row had been activated
- **WHEN** focus passes over such rows more quickly than the settle delay (for example while an arrow key is held down)
- **THEN** no intermediate row's content is loaded or rendered
- **WHEN** keyboard focus rests on a disclosure-only grouping row
- **THEN** the detail pane does not change

#### Scenario: First-letter typeahead

- **WHEN** the tree has focus and the user types a printable character
- **THEN** focus moves to the next visible row after the current one whose label starts with that character, comparing case-insensitively and wrapping past the end of the tree
- **AND** if no visible row label starts with that character, focus does not move

#### Scenario: Tree rows expose ARIA tree semantics

- **WHEN** the tree is rendered
- **THEN** the container exposes `role="tree"`, every row exposes `role="treeitem"` with an accurate `aria-level`, expandable rows expose `aria-expanded` reflecting their disclosure state, the selected row exposes `aria-selected="true"`, and nested child groups are wrapped in `role="group"` containers
- **AND** dim missing-artifact rows remain keyboard-focusable but expose `aria-disabled="true"` and do not respond to activation

#### Scenario: Focus survives the focused row disappearing

- **WHEN** a tree refresh (for example a filesystem cache event) removes the row that currently holds keyboard focus
- **THEN** focus falls back to the nearest surviving ancestor row derived from the removed row's hierarchical node ID, rather than being lost to the document body

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

### Requirement: Mermaid Diagram Rendering

The detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability (`mermaid` here, `svg` in the *SVG Fence Rendering* requirement, `math` in the *Mathematical Notation Rendering* requirement) SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

A rendered diagram SHALL derive its colours and fonts from the application's design tokens (see the *Design Token Layer* and *Typography System* requirements in the `visual-identity` capability) so that it reads as part of the same surface as the surrounding prose in both the light and dark schemes. This obligation extends to colours the diagram engine derives on its own for values the application does not map explicitly: the application SHALL inform the engine of the active scheme so that every derived colour is derived in the direction of that scheme, rather than under an assumed light palette. Diagram text SHALL remain legible against every filled surface the engine draws — including alternating table-row fills such as entity-relationship attribute rows, whose fills SHALL come from the design tokens' surface colours. When the operating system colour scheme changes while a diagram is visible, the diagram SHALL re-render so its colours follow the active scheme.

A rendered diagram SHALL be capped at the detail pane's content width while reading, and SHALL be openable in the maximized view described by the *Maximized Figure View* requirement, so that a diagram scaled down to fit a narrow pane remains readable.

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

#### Scenario: A diagram too wide for the pane remains readable

- **WHEN** an artifact contains a diagram whose natural width exceeds the detail pane's content width
- **THEN** it is displayed scaled down to fit the pane, as before
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

**Affordance.** Each maximizable figure SHALL present a control that opens the maximized view. The control SHALL be operable by keyboard as well as by pointer (see the *Shell Keyboard Operability* requirement). On a device that reports no hover capability it SHALL be rendered visibly at rest, and on a device whose primary pointer is coarse it SHALL present an enlarged hit area, per the *Essential Controls Are Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirements in the `touch-input` capability. The figure's inline presentation SHALL be unchanged by the presence of the control: a figure still fits the detail pane's width while reading, and the maximized view is an addition to that default rather than a replacement for it.

**Initial scale.** The maximized view SHALL open with the figure fully visible — scaled so that neither dimension exceeds the surface's content area, with the scale taken from whichever axis constrains it more. For a surface of extents $W_v \times H_v$ with padding $p$, displaying content of extents $W_c \times H_c$:

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

Relative links to markdown files (matched case-insensitively) SHALL be inert in v1, reserved for future in-app navigation. Fragment-only links and links with any other scheme (including `javascript:` and `file:`) SHALL be inert. Inert links SHALL carry a visual affordance distinguishing them from openable links, so a dead link reads as policy rather than breakage.

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

### Requirement: Mathematical Notation Rendering

The detail pane SHALL render GitHub-flavored mathematical notation as typeset formulas: an inline dollar-delimited expression (`$…$`) SHALL render as inline mathematics within the surrounding prose, a double-dollar-delimited expression (`$$…$$`) standing alone as its own paragraph — in either its single-line or multi-line block form — SHALL render as display (block) mathematics, a double-dollar expression embedded within surrounding prose SHALL render as inline mathematics, and a fenced code block whose info string is `math` SHALL render as display mathematics rather than as syntax-highlighted source. Mathematics rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend SHALL continue to present mathematical source as plain text.

Dollar delimiters SHALL NOT be recognised inside code spans or fenced code blocks (other than the `math` fence itself), so a literal dollar sign in backticked text — for example a `\\wsl$\<distro>` path — is never parsed as mathematics. A dollar sign with no valid closing delimiter SHALL render as a literal dollar sign.

Rendered mathematics SHALL inherit the surrounding text colour, so it follows the active colour scheme in both light and dark without any repainting or re-rendering machinery. Display mathematics wider than the pane's content width SHALL scroll horizontally within its own block rather than widening the artifact. Rendered mathematics SHALL carry a machine-readable representation (MathML) alongside the visual output so assistive technology can consume it. Rendering SHALL work without network access: the mathematics engine and its assets are part of the application bundle.

Invalid input SHALL degrade gracefully and locally: a dollar-delimited expression that is not valid mathematical source SHALL present its raw source in place with a quiet visual indication of the error, while the rest of the artifact renders normally; a `math` fence whose body cannot be rendered SHALL likewise present the fence's raw source with a quiet visual indication that the formula could not be rendered. Neither case SHALL blank or crash the pane.

Mathematics rendering SHALL run under a non-trusting posture so mathematical source cannot inject active content: commands that would emit hyperlinks, external references, or scripts (for example `\href`) SHALL NOT produce live links, fetch external resources, or execute.

#### Scenario: Inline math renders within prose

- **WHEN** an artifact contains an inline dollar-delimited expression such as `$O(n \log n)$` in a sentence
- **THEN** the detail pane renders it as typeset inline mathematics flowing with the surrounding text
- **AND** the raw LaTeX source is not shown

#### Scenario: Display math renders as a block

- **WHEN** an artifact contains a double-dollar-delimited expression standing alone as its own paragraph (single-line or multi-line block form) or a fenced code block with the `math` info string
- **THEN** the detail pane renders it as display mathematics in its own block
- **AND** a double-dollar expression embedded mid-sentence renders as inline mathematics instead
- **AND** a formula wider than the pane's content width scrolls horizontally within that block without widening the artifact

#### Scenario: Dollar signs in code are never math

- **WHEN** an artifact contains dollar signs inside a code span or a fenced code block in another language — for example `` `\\wsl$\Ubuntu\home` `` or `` `releases/${tag}.md` ``
- **THEN** they render as literal dollar signs, unchanged
- **AND** a dollar sign in prose with no valid closing delimiter renders as a literal dollar sign

#### Scenario: Invalid inline math degrades in place

- **WHEN** an artifact contains a dollar-delimited expression whose content is not valid mathematical source
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

While the detail pane's target is an OpenSpec artifact, the pane SHALL render a **change-identity header** above the artifact's markdown, naming the change the artifact belongs to. The header applies to the artifact target only: the commit detail view, the Dashboard, the workspace file browser, the Archive view, and the Settings view each carry their own header and SHALL be unaffected.

**Content.** The header SHALL display the change's **directory name** — the `openspec/changes/<name>` folder name, which is the identifier a user hands to external tooling — rendered verbatim and in full, with no truncation, ellipsis, or transformation. It SHALL NOT substitute the change's `proposal.md` title, which the tree already shows (see *Two-Line Sole-Change-Row Layout*) and which is not the change's filesystem identity. Following the name, the header SHALL show the owning worktree's branch as an outlined chip (per *visual-identity → Outlined Chip Badges*). When the artifact belongs to a flat (non-git) workspace, or the worktree's branch is otherwise not known, no chip SHALL be rendered and the header SHALL show the name alone. An **archived** change SHALL render no chip: it has no live worktree, and the worktree path its artifact is read from routinely hosts other, active changes whose branch was never the archived change's.

**Branch chip colour.** The branch chip SHALL be **tinted to the owning workspace's palette colour** — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour — so the artifact under the header reads as belonging to the workspace it came from. The workspace whose colour applies is the one owning the worktree the artifact was read from, which is the same worktree whose branch the chip names; no other workspace's colour SHALL be substituted. When the owning workspace has no configured palette colour, the chip SHALL render in the neutral ink it renders in today, and SHALL NOT fall back to an arbitrary or derived colour.

The header's chip and the tree's chip naming the same branch of the same change SHALL render **identically** — the same tint, weight, and treatment (see the *Two-Line Sole-Change-Row Layout* requirement, which specifies the tree's chip). The two surfaces are visible simultaneously, so a single value SHALL NOT be presented two ways. This equivalence is a property of the rendered result and SHALL hold for every palette colour and for the untinted case, so that changing how one surface renders the chip cannot leave the other behind.

**Last changed.** The header SHALL report **when the artifact currently rendered last changed**, as an interval elapsed since that moment, expressed in relative terms (for example `just now`, `9m ago`, `12d ago`) rather than as an absolute clock time. The detail pane is already refreshed live (see *Reactive Updates from Filesystem*), so this value does not report whether the view is current; it reports how long the artifact has stood.

The label SHALL use the **same relative-time vocabulary** as every other surface in the application that presents an elapsed time — the tree's per-instance modification time (see *Multi-Instance Child Row* and *Two-Line Sole-Change-Row Layout*) and the Dashboard's relative archive time (see the `dashboard` capability). The header and the tree are visible simultaneously and routinely describe the same change, so one kind of value SHALL NOT be spelled two ways on one screen. This equivalence is a property of the rendered result and SHALL hold at every tier of the vocabulary, so that changing how one surface words an interval cannot leave the others behind.

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

**Anchoring.** Because the header occupies the top of the pane's scroll port, scroll anchors (see *Section and Task Scroll Anchors*) SHALL account for its height: a section or task scrolled to SHALL come to rest fully visible below the header, never underneath it. The height SHALL be taken from the rendered header rather than from a fixed constant, because the change name renders in full and therefore wraps at narrow pane widths — and because the macOS clearance changes that height. Any clearance SHALL therefore be inside the measured element.

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

