## ADDED Requirements

### Requirement: Settings Menu Item

On macOS, the application submenu SHALL include a "Settings…" item with the Cmd+, accelerator. It SHALL sit between "About SpecForge" and the Services item, with a separator on either side, where macOS places an application's settings item.

Activating the item SHALL show and focus the main window, including when the main window is hidden or a reader window is focused, and SHALL show the Settings view there:

- When the main window is not already showing the Settings view, it SHALL open at the Workspaces group (see the *Entry Points Open the Workspaces Group* requirement in the `settings-view` capability).
- When the main window is already showing the Settings view, the group already shown SHALL remain shown.

The item SHALL reach the frontend by emitting an event to the webview. The event name SHALL be defined once on the Rust side and mirrored in the frontend's shared types, consistent with the View submenu's pane toggles (see the *View Submenu Pane Toggles* requirement). A single keypress of Cmd+, SHALL open the Settings view exactly once: no in-webview handling of the same key combination SHALL also act on it.

Because the custom menu is macOS-only (see the *Custom macOS Application Menu* requirement), no Settings… item exists on Windows or Linux. Those platforms reach Settings through the sidebar Settings entrypoint.

#### Scenario: Settings… opens the Workspaces group

- **WHEN** the main window is showing an artifact and the user selects SpecForge → Settings… on macOS
- **THEN** the main window shows the Settings view at its Workspaces group

#### Scenario: Settings… reaches a hidden main window

- **WHEN** the main window is hidden and a reader window is focused on macOS
- **AND** the user presses Cmd+,
- **THEN** the main window is shown and focused
- **AND** it shows the Settings view

#### Scenario: Settings… keeps the group already shown

- **WHEN** the main window is showing the Settings view's Integrations group and the user presses Cmd+, on macOS
- **THEN** the Integrations group remains shown

#### Scenario: One keypress opens Settings once

- **WHEN** the user presses Cmd+, once with the main window focused on macOS
- **THEN** the Settings view is opened by exactly one navigation

#### Scenario: The item sits after About

- **WHEN** the application menu is installed on macOS
- **THEN** the application submenu's first item is "About SpecForge"
- **AND** "Settings…" follows it, separated from it and from the Services item
