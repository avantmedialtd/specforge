# tray-indicator Specification

## Purpose

Defines the always-on operating-system tray presence of the application, the badge that summarises active OpenSpec changes across registered workspaces, and the desktop notifications dispatched on structural change events.

## Requirements

### Requirement: Tray Icon Presence

The application SHALL present an icon in the operating-system menu bar (macOS), system tray (Windows), or status notifier area (Linux) whenever the application process is running, independent of whether the main window is open or hidden.

#### Scenario: Icon appears at startup

- **WHEN** the application launches
- **THEN** an icon is added to the operating-system tray area within one second of process start

#### Scenario: Icon persists across window state

- **WHEN** the user closes the main window while the application keeps running
- **THEN** the tray icon remains visible

#### Scenario: Icon disappears on quit

- **WHEN** the user issues an explicit quit command (e.g. Cmd-Q on macOS)
- **THEN** the tray icon is removed from the tray area

### Requirement: Active-Change Badge

The tray icon SHALL display a badge whose value equals the total count of non-archived changes across all registered workspaces. A non-archived change is any directory directly under `openspec/changes/` whose immediate parent is not `openspec/changes/archive/`. The badge MUST be hidden when the count is zero.

#### Scenario: Badge reflects aggregate count

- **WHEN** the registered workspaces contain three non-archived changes in aggregate
- **THEN** the tray badge displays "3"

#### Scenario: Badge hidden when no active changes

- **WHEN** every registered workspace has zero non-archived changes
- **THEN** the tray badge is not displayed

#### Scenario: Badge decrements on archive

- **WHEN** an existing non-archived change is moved into the `openspec/changes/archive/` directory on disk
- **THEN** the badge value decreases by one within the watcher debounce window

#### Scenario: Badge increments on new change

- **WHEN** a new directory is created under `openspec/changes/` of a registered workspace
- **THEN** the badge value increases by one within the watcher debounce window

### Requirement: Click to Focus Main Window

Clicking the tray icon SHALL bring the main application window to the foreground, opening it if it is currently hidden.

#### Scenario: Click opens hidden window

- **WHEN** the main window is hidden
- **AND** the user clicks the tray icon
- **THEN** the main window is shown and given focus

#### Scenario: Click focuses backgrounded window

- **WHEN** the main window is open but behind other applications
- **AND** the user clicks the tray icon
- **THEN** the main window is raised to the foreground

### Requirement: Desktop Notification on New Change

The application SHALL display a desktop notification when a new change directory appears in any registered workspace's `openspec/changes/` directory.

#### Scenario: New change emits notification

- **WHEN** a new directory is created under `openspec/changes/` of a registered workspace
- **THEN** a desktop notification is dispatched identifying the workspace and the change identifier

### Requirement: Desktop Notification on Archive Transition

The application SHALL display a desktop notification when an existing change transitions between active (`openspec/changes/<id>/`) and archived (`openspec/changes/archive/<id>/`) locations.

#### Scenario: Change moved to archive

- **WHEN** a change directory is moved from `openspec/changes/<id>/` to `openspec/changes/archive/<id>/`
- **THEN** a desktop notification is dispatched indicating the change has been archived

### Requirement: No Notification on File Edit

The application SHALL NOT dispatch a desktop notification for ordinary file edits within an existing change directory. Only structural transitions (new change, archive move) trigger notifications.

#### Scenario: Editing tasks.md is silent

- **WHEN** a file inside an existing non-archived change directory is modified
- **THEN** no desktop notification is dispatched
