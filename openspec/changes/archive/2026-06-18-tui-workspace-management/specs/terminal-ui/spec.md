## MODIFIED Requirements

### Requirement: Settings Screen

The interactive frontend SHALL provide a Settings screen that presents the application settings the terminal frontend can act on. The screen SHALL present a set of toggle rows — each showing its current on/off state — and a Workspaces section listing the user-registered workspaces with controls to add, remove, rename, and recolor them. The first version's toggles SHALL include the gamification master switch and the Claude usage-quota opt-in. The user SHALL be able to flip each toggle, and the change SHALL be persisted immediately to the shared application settings without a separate save action. A setting changed from this screen SHALL take effect in the running frontend without requiring a restart. The behaviour of the Workspaces section is specified by the Workspace Management from the Terminal requirement.

#### Scenario: Settings screen lists actionable toggles

- **WHEN** the Settings screen is shown
- **THEN** a row is rendered for the gamification switch and for the Claude usage-quota opt-in
- **AND** each row shows whether that setting is currently on or off

#### Scenario: Settings screen lists registered workspaces

- **WHEN** the Settings screen is shown
- **THEN** a Workspaces section lists every user-registered workspace with its name and folder path
- **AND** an add-workspace control is shown

#### Scenario: Toggling a setting persists immediately

- **WHEN** the user flips a toggle on the Settings screen
- **THEN** the new value is written to the shared application settings without a separate save action
- **AND** the value is still in effect when the frontend is restarted

#### Scenario: Toggling gamification updates the gamified surfaces

- **WHEN** the user toggles the gamification switch on the Settings screen
- **THEN** the gamified surfaces (Dashboard, Season, Garden) reflect the new state without restarting the frontend

#### Scenario: Toggling the quota opt-in updates the title-bar gauge

- **WHEN** the user disables the Claude usage-quota opt-in on the Settings screen
- **THEN** the title-bar quota gauge is cleared without restarting the frontend
- **AND** re-enabling it shows the gauge again once the quota poller next refreshes

### Requirement: Read-Only Operation

The frontend SHALL NOT modify the contents of any registered workspace. Browsing, dashboard, season, and settings views SHALL be presentation-only with respect to workspace files. The frontend MAY persist application configuration to the shared configuration directory — including application settings, the registered-workspace list, and per-workspace presentation overrides (display name and colour) — which lives outside any registered workspace; doing so SHALL NOT constitute modifying a workspace. Registering or unregistering a workspace changes only the application's record of which folders to observe; it SHALL NOT create, modify, or delete any file inside the affected folder.

#### Scenario: Browsing does not alter workspace files

- **WHEN** the user navigates workspaces, reads artifacts, and views the dashboard and season screens
- **THEN** no files inside any registered workspace are created, modified, or deleted by the frontend

#### Scenario: Settings writes target app config, not workspaces

- **WHEN** the user toggles a setting from the Settings screen
- **THEN** only the application settings file in the shared configuration directory is written
- **AND** no files inside any registered workspace are created, modified, or deleted

#### Scenario: Registering a workspace does not write inside the workspace

- **WHEN** the user adds or removes a workspace from the Settings screen
- **THEN** only the application's configuration directory (the registry and presentation stores) is written
- **AND** no files inside the added or removed folder are created, modified, or deleted

## ADDED Requirements

### Requirement: Workspace Management from the Terminal

The interactive frontend SHALL allow the user to manage the registered-workspace set from the Settings screen: add a workspace by path, remove a user-registered workspace, and set a workspace's display name and palette colour. These operations SHALL go through the shared application service so the registry, the filesystem watcher, and the presentation store stay consistent, and their effects SHALL appear in the running frontend without a restart. The Settings workspace list SHALL present the user-registered workspaces only; auto-discovered worktrees SHALL NOT appear as manageable rows.

#### Scenario: Add a workspace by path

- **WHEN** the user invokes the add-workspace control and enters the path of a folder that contains an `openspec/` subdirectory
- **THEN** the folder is registered, a filesystem watcher is established for it, and it appears in the Settings workspace list and the Browse tree without a restart

#### Scenario: Invalid path is rejected with a message

- **WHEN** the user enters a path that does not exist, is not a directory, or lacks an `openspec/` subdirectory
- **THEN** the workspace is not added
- **AND** a message indicates why the folder is not a valid OpenSpec workspace
- **AND** the add prompt remains open for correction

#### Scenario: Remove a workspace with cascade awareness

- **WHEN** the user removes a user-registered workspace and confirms
- **THEN** the workspace and any worktrees discovered through it are unregistered, their watchers are disposed, and they disappear from the Settings list and the Browse tree without a restart

#### Scenario: Rename a workspace

- **WHEN** the user sets a display name for a workspace
- **THEN** the name is persisted to the presentation store and shown in the Settings list and the Browse tree
- **AND** clearing the name reverts the workspace to its default basename

#### Scenario: Set a workspace colour

- **WHEN** the user selects a palette colour for a workspace
- **THEN** the colour token is persisted to the presentation store and the workspace's row is tinted accordingly in the Browse tree
- **AND** selecting "none" clears the colour back to the default untinted row
