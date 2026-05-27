# spec-browser Delta — Move Settings entrypoint to sidebar footer

## ADDED Requirements

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
