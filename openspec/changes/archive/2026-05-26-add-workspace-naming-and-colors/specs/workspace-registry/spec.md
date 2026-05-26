## ADDED Requirements

### Requirement: Workspace Presentation Persistence

The application SHALL persist a per-top-level-row presentation entry (display name and palette colour) in a presentation store separate from the workspace registry. Presentation entries SHALL be keyed by the identity of the top-level row they decorate: a canonical workspace path for a flat (non-git) workspace, and a canonical git common directory for a repository group. Presentation entries SHALL survive application restarts. When the user unregisters the last user-registered workspace associated with a given key (a flat workspace's path, or any user-registered workspace whose repository identifier matches a repo-keyed entry), the presentation entry for that key SHALL be removed.

The presented display name MUST be normalised so that an empty string is stored as absent. The presented colour MUST be one of a fixed curated palette of tokens (`indigo`, `blue`, `teal`, `green`, `amber`, `orange`, `rose`, `purple`) or absent. No other colour values SHALL be accepted by the store.

#### Scenario: Presentation entry persists across restart

- **WHEN** the user sets a display name and colour for a registered workspace
- **AND** quits and relaunches the application
- **THEN** the presentation entry is restored from disk
- **AND** the workspace continues to render with the chosen display name and colour

#### Scenario: Presentation entry cleaned up when underlying workspace is unregistered

- **WHEN** the user unregisters a flat workspace that has a presentation entry
- **THEN** the presentation entry keyed by that workspace's path is removed from the store
- **AND** re-registering the same path afterwards starts with the default display name and no colour

#### Scenario: Repo-keyed presentation cleaned up when the last user-registered workspace for the repo is unregistered

- **WHEN** the user has a repository with two user-registered workspaces and a presentation entry keyed by that repository
- **AND** the user unregisters one of the two workspaces
- **THEN** the presentation entry is preserved (the repository still has a user-registered workspace)
- **WHEN** the user unregisters the remaining user-registered workspace for that repository
- **THEN** the presentation entry keyed by the repository's common directory is removed from the store

#### Scenario: Empty display-name input falls back to default

- **WHEN** the user submits an empty display name from the Settings view
- **THEN** the persisted display name is stored as absent (not an empty string)
- **AND** the workspace renders with its default name (the folder basename for flat workspaces or the main worktree's basename for repository groups)

#### Scenario: Invalid colour value is rejected

- **WHEN** an attempt is made to set a colour value that is not one of the curated palette tokens and is not absent
- **THEN** the presentation store rejects the update with an error
- **AND** the existing entry, if any, is left unchanged

### Requirement: Presentation Fields on Listed Workspaces

The data returned by the workspace listing command and the aggregated repo-view command SHALL include the configured display name (or absent if none) and colour (or absent if none) for each top-level row. When no presentation entry exists for a row, both fields SHALL be absent and consumers SHALL render the row exactly as they did before the presentation store was introduced.

#### Scenario: Workspace list includes display name and colour

- **WHEN** the frontend requests the list of registered workspaces
- **THEN** each workspace entry includes its configured display name (or null) and colour token (or null)
- **AND** workspaces with no presentation entry return null for both fields

#### Scenario: Repo view includes display name and colour for the group

- **WHEN** the frontend requests the aggregated repo-and-flat view
- **THEN** each repo group entry includes its configured display name (or null) and colour token (or null)
- **AND** flat workspace entries in the same view also include their per-workspace display name and colour

## MODIFIED Requirements

### Requirement: Settings View

The main window SHALL include a settings view, reachable from a discoverable affordance in the main window chrome, that surfaces: the registered-workspaces list with add and remove controls, a per-workspace inline display-name field, a per-workspace palette swatch picker that accepts one of the curated palette tokens or "none", a launch-on-login toggle, and a notifications-enabled toggle.

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

#### Scenario: Inline display-name field renames the workspace

- **WHEN** the user edits the inline display-name field for a listed workspace and commits the change
- **THEN** the new display name is persisted to the presentation store under the workspace's row-identity key
- **AND** subsequent renderings of that workspace in the Settings list and the tree pane show the new name
- **AND** clearing the field reverts the workspace to its default name (the folder basename or, for a repository group, the main worktree's basename)

#### Scenario: Palette swatch picker sets the workspace colour

- **WHEN** the user selects a palette swatch for a listed workspace
- **THEN** the chosen colour token is persisted to the presentation store under the workspace's row-identity key
- **AND** the tree pane re-renders the workspace's parent row with the corresponding tint
- **WHEN** the user selects the "none" swatch
- **THEN** the persisted colour is cleared
- **AND** the workspace's parent row reverts to the default untinted background
