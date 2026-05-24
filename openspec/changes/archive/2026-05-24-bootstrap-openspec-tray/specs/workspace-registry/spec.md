# workspace-registry

## ADDED Requirements

### Requirement: Manual Workspace Registration

The application SHALL allow the user to register a workspace by selecting a folder on disk that contains an `openspec/` subdirectory. A folder lacking an `openspec/` subdirectory MUST be rejected with a user-visible message.

#### Scenario: Valid folder is registered

- **WHEN** the user opens the settings view and selects a folder containing an `openspec/` subdirectory
- **THEN** the folder is added to the registered-workspaces list
- **AND** the folder appears in the tree pane as a top-level workspace node

#### Scenario: Invalid folder is rejected

- **WHEN** the user selects a folder that does not contain an `openspec/` subdirectory
- **THEN** the folder is not added to the registered-workspaces list
- **AND** a message indicates the folder is not a valid OpenSpec workspace

### Requirement: Workspace Removal

The application SHALL allow the user to remove a workspace from the registered-workspaces list. Removal MUST dispose any filesystem watcher associated with the removed workspace and update the badge accordingly.

#### Scenario: Workspace removed via settings

- **WHEN** the user removes a workspace from the registered list in the settings view
- **THEN** the workspace is no longer shown in the tree pane
- **AND** the badge count decreases by the number of non-archived changes that workspace contributed

### Requirement: Registration Persistence

The list of registered workspaces SHALL be persisted to a config file managed by `openspec-core` and restored on application startup.

#### Scenario: Registrations survive restart

- **WHEN** the user registers a workspace
- **AND** quits and relaunches the application
- **THEN** the workspace is still present in the registered-workspaces list
- **AND** the tree pane shows the workspace's non-archived changes

### Requirement: Filesystem Watching of Registered Workspaces

For each registered workspace, the application SHALL watch the workspace's `openspec/changes/` directory for additions, removals, and modifications, using a debounced event stream to coalesce bursts of filesystem events.

#### Scenario: Watcher established on registration

- **WHEN** a workspace is added to the registered list
- **THEN** a filesystem watcher is established on that workspace's `openspec/changes/` directory

#### Scenario: Watcher disposed on removal

- **WHEN** a workspace is removed from the registered list
- **THEN** the filesystem watcher for that workspace is disposed

#### Scenario: Burst of edits is coalesced

- **WHEN** multiple files inside one registered workspace are modified within the debounce window
- **THEN** the cache and UI receive a single coalesced update event, not one event per file

### Requirement: In-Memory Cache of Parsed State

The application SHALL maintain an in-memory cache of parsed OpenSpec state (changes, artifacts, sections, tasks) for each registered workspace. The cache MUST be kept consistent with on-disk state by the watcher and MUST be the source of truth for the tree pane and badge — neither queries the filesystem directly.

#### Scenario: Cache reflects on-disk change within debounce window

- **WHEN** a file under a registered workspace's `openspec/changes/` directory is modified
- **THEN** the in-memory cache for that workspace is updated within the watcher debounce window
- **AND** subsequent reads from the tree pane and badge use the updated cache

### Requirement: Settings View

The main window SHALL include a settings view, reachable from a discoverable affordance in the main window chrome, that surfaces: the registered-workspaces list with add and remove controls, a launch-on-login toggle, and a notifications-enabled toggle.

#### Scenario: Settings view shows registered workspaces

- **WHEN** the user opens the settings view
- **THEN** every currently registered workspace is listed with its folder path and a remove control
- **AND** an add-workspace control is visible

#### Scenario: Launch-on-login toggle is persisted and applied

- **WHEN** the user enables the launch-on-login toggle
- **THEN** the application registers itself for launch at the next system login via the operating system's autostart mechanism
- **AND** the toggle state is persisted across application restarts

#### Scenario: Notifications-enabled toggle suppresses notifications

- **WHEN** the user disables the notifications-enabled toggle
- **THEN** subsequent new-change and archive-transition events do not dispatch desktop notifications
- **AND** the toggle state is persisted across application restarts

### Requirement: Missing Workspace Handling

When a registered workspace's folder no longer exists on disk, the application SHALL mark that workspace as missing in the settings view without crashing the watcher or removing the workspace from the registered list.

#### Scenario: Missing folder shown as such

- **WHEN** a registered workspace's folder is deleted while the application is running
- **THEN** the workspace appears in the settings view with a "missing" indicator
- **AND** the tree pane shows the workspace with no children (or hides it pending user action)
- **AND** no further filesystem watching is attempted on the missing path until the user re-registers it or removes it
