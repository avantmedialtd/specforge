# spec-browser Delta

## MODIFIED Requirements

### Requirement: Master-Detail Layout

The main application window SHALL present a master-detail layout of two primary panes — a tree-navigation pane on the left and a content-rendering (detail) pane in the center — plus an optional commit-graph rail on the far right (see the *Commit-Graph Rail Pane* requirement in the `commit-graph` capability). Resizable dividers separate the panes. The tree pane and the rail are each independently hideable (see the *Side-Pane Visibility Toggles* requirement); the detail pane is always visible.

The detail (center) pane SHALL render one of four targets: an OpenSpec artifact's markdown, a commit's detail view when a commit is selected in the rail, the **Dashboard** (see the *Dashboard Home Surface* requirement in the `dashboard` capability), or the **Archive view** (see the *Archive View* requirement in the `archive-browser` capability) when the Archive entrypoint is active. The Dashboard SHALL be the default target: it is rendered at startup and whenever no artifact and no commit is selected and the Archive view is not open, in place of any "nothing selected" placeholder. The Archive view and the Settings view are modal pane targets toggled from their sidebar entrypoints; while either is open it takes precedence over the artifact/commit/Dashboard target, and closing it returns the pane to whichever of those was selected most recently. The tree drives the artifact target and the rail drives the commit target.

#### Scenario: Panes visible at startup

- **WHEN** the user opens the main window for the first time
- **THEN** the tree pane and the detail pane are visible side by side
- **AND** the commit-graph rail is visible on the far right
- **AND** the detail pane renders the Dashboard (no artifact or commit having been selected, and the Archive view not open)
- **AND** the dividers between the panes can be dragged to adjust their widths

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

## ADDED Requirements

### Requirement: Side-Pane Visibility Toggles

The tree-navigation pane (sidebar) and the commit-graph rail SHALL each be independently hideable and restorable, in both the desktop application and the served web UI. Any combination of hidden/shown SHALL be reachable; with both side panes hidden the detail pane SHALL occupy the full window width. The detail pane itself SHALL NOT be hideable.

Each visibility SHALL be togglable by keyboard: Cmd+B (macOS) / Ctrl+B (Windows, Linux) for the sidebar, and Cmd+Alt+B (macOS) / Ctrl+Alt+B (Windows, Linux) for the rail, with the same bindings active in the served web UI.

Each visible side pane SHALL display a collapse affordance (a chevron control) at its top. While a side pane is hidden, a restore affordance SHALL be displayed in the corresponding top corner of the detail pane (top-left for the sidebar, top-right for the rail), so that restoring a pane never requires a keyboard shortcut, a menu, or an application restart.

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
