# dock-indicator Delta — Add Dock Badge with Active Change Count

## ADDED Requirements

### Requirement: Dock Tile Badge Reflects Active Logical Change Count (macOS)

On macOS the application SHALL render a numeric badge on its Dock tile whose value equals the count of non-archived *logical changes* across all tracked workspaces — i.e. the value returned by `WatcherManager::total_active_logical_count()`. The badge SHALL be hidden when the count is zero. The badge SHALL be applied via the main webview window's `Window::set_badge_count`; the operating system propagates the same badge to every visual surface that renders the application's Dock tile, including the CMD+Tab application switcher.

The Dock badge SHALL update in response to every `CacheEvent` emitted by `WatcherManager`, within the same debounce window as the menu-bar tray badge. The Dock badge value at any instant SHALL equal the tray badge value at the same instant. The two indicators MUST NOT drift apart at any point in the lifetime of the application.

The badge SHALL be visible regardless of whether the main window is visible, hidden, minimised, or in the background. Window-visibility lifecycle MUST NOT affect Dock-badge updates.

#### Scenario: Badge appears at startup with the active count

- **WHEN** the application launches on macOS with at least one registered workspace that has one or more non-archived logical changes
- **THEN** within one second of the main window resolving, the Dock tile shows a badge whose digit equals the total active logical-change count across all tracked workspaces
- **AND** the badge digit equals the tray badge digit at the same instant

#### Scenario: Badge increments on new logical change

- **WHEN** the Dock badge currently displays a count `n`
- **AND** a change directory with a new `(repository_id, change_name)` tuple (not present in any other worktree of its repository) is created in a tracked workspace
- **THEN** within the watcher debounce window the Dock badge updates to `n + 1`
- **AND** the tray badge updates to `n + 1` at the same time

#### Scenario: Badge decrements on final-instance archive

- **WHEN** the Dock badge currently displays a count `n`
- **AND** the last non-archived instance of a logical change is moved into `openspec/changes/archive/`
- **THEN** within the watcher debounce window the Dock badge updates to `n - 1`
- **AND** the tray badge updates to `n - 1` at the same time

#### Scenario: Badge unchanged when non-final instance archived

- **WHEN** the Dock badge currently displays a count `n`
- **AND** one instance of a multi-instance logical change is moved into `openspec/changes/archive/`
- **AND** at least one other instance of the same logical change remains active in another worktree
- **THEN** the Dock badge value remains `n`
- **AND** matches the tray badge value, which also remains `n`

#### Scenario: Badge hidden when no active changes remain

- **WHEN** every tracked workspace has zero non-archived logical changes
- **THEN** the Dock tile shows no badge digit
- **AND** the tray badge is also hidden (per the existing `tray-indicator` capability)

#### Scenario: Badge visible in CMD+Tab application switcher

- **WHEN** the Dock badge currently displays a non-zero digit
- **AND** the user holds Cmd and presses Tab to invoke the macOS application switcher
- **THEN** the SpecForge tile in the switcher displays the same badge digit as the Dock tile

#### Scenario: Badge updates while the main window is hidden

- **WHEN** the user has closed the main window (which hides rather than destroys it)
- **AND** a `CacheEvent` changes the active logical-change count
- **THEN** the Dock badge updates to the new count within the watcher debounce window
- **AND** the update completes without the main window being shown or made visible

#### Scenario: Initial badge persists across application restarts

- **WHEN** the application is launched, then quit with Cmd-Q, then relaunched
- **AND** at relaunch time at least one tracked workspace has non-archived logical changes
- **THEN** on the relaunched application the Dock badge is present on first paint
- **AND** the badge digit equals the total active logical-change count

### Requirement: Dock Badge Code Path Excluded On Non-macOS

The Dock-badge module SHALL be excluded from compilation entirely on Windows and Linux targets. The application MUST NOT call `Window::set_badge_count` from this capability's code path on any non-macOS platform. A Windows or Linux Dock-tile equivalent (overlay icon on Windows, launcher metadata on Linux) is out of scope for this capability and SHALL be specified in a separate future capability if and when added.

#### Scenario: Module not present in non-macOS builds

- **WHEN** the application is compiled for any non-macOS target
- **THEN** the `dock_badge` module is not compiled into the binary
- **AND** no call to `Window::set_badge_count` is emitted by code in this capability's call path

#### Scenario: macOS build wires the updater

- **WHEN** the application is compiled for macOS and runs through its startup sequence
- **THEN** exactly one updater task is spawned that subscribes to the `WatcherManager` broadcast and applies the Dock badge on every `CacheEvent`
