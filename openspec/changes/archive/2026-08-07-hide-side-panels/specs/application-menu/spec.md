# application-menu Delta

## ADDED Requirements

### Requirement: View Submenu Pane Toggles

On macOS, the custom application menu SHALL include a View submenu containing two items: "Toggle Sidebar" with the Cmd+B accelerator, and "Toggle Commit Rail" with the Cmd+Alt+B accelerator. Activating an item SHALL toggle the visibility of the corresponding pane in the main window (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability), showing the main window first if it is hidden.

The menu items SHALL reach the frontend by emitting an event to the webview; the event name SHALL be defined once on the Rust side and mirrored in the frontend's shared types, consistent with the existing cache-event bridge.

A single keypress of an accelerator SHALL produce exactly one toggle: the native menu accelerator and any in-webview handling of the same key combination SHALL NOT both fire for one keypress.

Because the custom menu is macOS-only (see the *Custom macOS Application Menu* requirement), no View submenu exists on Windows or Linux; those platforms rely on the keyboard bindings and on-screen affordances specified in the `spec-browser` capability.

#### Scenario: View menu toggles the sidebar

- **WHEN** the user selects View → Toggle Sidebar on macOS
- **THEN** the main window's sidebar visibility flips
- **AND** the commit rail's visibility is unchanged

#### Scenario: View menu toggles the commit rail

- **WHEN** the user selects View → Toggle Commit Rail on macOS
- **THEN** the main window's commit-rail visibility flips
- **AND** the sidebar's visibility is unchanged

#### Scenario: Accelerator fires exactly one toggle

- **WHEN** the user presses Cmd+B once with the main window focused on macOS
- **THEN** the sidebar's visibility flips exactly once
