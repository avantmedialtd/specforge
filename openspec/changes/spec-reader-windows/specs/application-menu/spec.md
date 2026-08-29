# application-menu Delta — Reader Windows for Workspace Documents

## MODIFIED Requirements

### Requirement: Standard Edit and Window Items Preserved

Because installing a custom menu replaces Tauri's auto-default menu, the custom menu SHALL reconstruct the standard editing and window items so that system shortcuts continue to work. The Edit submenu SHALL include Undo, Redo, Cut, Copy, Paste, and Select All. The Window submenu SHALL include Minimize and Close, and SHALL be registered as the macOS Windows menu (via the framework's window-submenu id) so the system attaches the standard Windows-menu role.

The Close item SHALL be the framework's predefined close-window action, which issues a close request to the focused window. It SHALL NOT branch on which window is focused. Each window kind's response to a close request is already decided by that window's own event handling: the main window intercepts the request and hides, so that the tray indicator and the filesystem watcher keep running (see the *Window State Persistence* requirement in the `spec-browser` capability and the tray behaviour in the `tray-indicator` capability), while a reader window installs no such interception and is destroyed (see the *Dismissing a Reader Window Destroys It* requirement in the `reader-window` capability). Placing that decision in the menu as well would duplicate it in a second location, where it could silently disagree with the first.

#### Scenario: Text-editing shortcuts work in app inputs

- **WHEN** a text input is focused in the application (for example the workspace-rename field in Settings) on macOS
- **THEN** Cut, Copy, Paste, and Select All operate via their standard keyboard shortcuts
- **AND** Undo and Redo are available

#### Scenario: Minimize shortcut works

- **WHEN** the main window is focused on macOS
- **THEN** the standard Minimize shortcut minimizes the window

#### Scenario: Close hides the main window rather than destroying it

- **WHEN** the main window is focused on macOS and the user invokes Close
- **THEN** the main window is hidden
- **AND** SpecForge continues running, with its tray indicator and filesystem watcher intact

#### Scenario: Close destroys a focused reader window

- **WHEN** a reader window is focused on macOS and the user invokes Close
- **THEN** that reader window is destroyed
- **AND** the main window's visibility is unaffected

#### Scenario: The menu does not decide close behaviour per window

- **WHEN** the Close item is invoked
- **THEN** it issues a close request to the focused window without inspecting which window that is
- **AND** the differing outcomes follow from each window's own close-request handling

#### Scenario: Window submenu carries the macOS Windows-menu role

- **WHEN** the application menu is installed on macOS
- **THEN** the Window submenu is registered as the system Windows menu
- **AND** the system-managed Windows-menu behaviour (Zoom, Bring All to Front, the open-window list) is attached rather than lost by replacing the auto-default menu

#### Scenario: Open reader windows appear in the system window list

- **WHEN** one or more reader windows are open on macOS
- **THEN** each appears in the Window submenu's system-managed open-window list alongside the main window
