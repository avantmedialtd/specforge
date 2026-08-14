## MODIFIED Requirements

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
