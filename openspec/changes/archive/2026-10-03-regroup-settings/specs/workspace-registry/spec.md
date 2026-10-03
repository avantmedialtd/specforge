## MODIFIED Requirements

### Requirement: Settings View

The main window SHALL include a settings view, reachable from a discoverable affordance in the main window chrome. Its structure (the groups it is divided into, how they are navigated, how each setting is laid out, and the rule by which settings persist) is specified by the `settings-view` capability.

Its Workspaces group SHALL surface:

- the registered-workspaces list with add and remove controls;
- a per-workspace inline display-name field;
- a per-workspace palette swatch picker that accepts one of the curated palette tokens or "none";
- a per-workspace enabled/disabled toggle.

Its Desktop app group SHALL surface a launch-on-login toggle and a notifications-enabled toggle.

The enabled/disabled toggle SHALL be the only surface from which a workspace is
disabled or re-enabled; no tree-pane or window-chrome affordance advertises or
alters the disabled state.

A per-workspace control whose change cannot be persisted SHALL report the failure
on the control the user operated, and SHALL continue to show the stored state
rather than the attempted one. Reporting the failure only to a developer console
does not satisfy this: the desktop and terminal frontends have no console the user
can see, so a rejected write would otherwise be indistinguishable from a control
that silently does nothing.

Where a toggle governs a repository group with more than one user-registered
worktree, the settings view SHALL make that shared scope legible on each affected
row, before the toggle is used. The rows are per registered folder while the flag
is per repository, so a user who operates one row moves the others; that
many-to-one relationship SHALL be visible rather than inferred from the result.

#### Scenario: Settings view shows registered workspaces

- **WHEN** the user opens the settings view
- **THEN** its Workspaces group is shown
- **AND** every currently registered workspace is listed with its folder path and a remove control
- **AND** an add-workspace control is visible

#### Scenario: Launch-on-login toggle is persisted and applied

- **WHEN** the user enables the launch-on-login toggle in the settings view's Desktop app group
- **THEN** the application registers itself for launch at the next system login via the operating system's autostart mechanism
- **AND** the toggle state is persisted across application restarts

#### Scenario: Notifications-enabled toggle suppresses notifications

- **WHEN** the user disables the notifications-enabled toggle in the settings view's Desktop app group
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

#### Scenario: Disable toggle removes the workspace from the tree

- **WHEN** the user switches a listed workspace's toggle to disabled
- **THEN** the workspace remains listed in the settings view, marked as disabled
- **AND** the tree pane no longer shows a top-level row for it
- **WHEN** the user switches the toggle back to enabled
- **THEN** the workspace reappears in the tree pane in its original position

#### Scenario: Disabling from one row of a repository updates its siblings

- **WHEN** a repository has two user-registered worktrees listed in the settings view
- **AND** the user disables the repository from one of those rows
- **THEN** both rows show the disabled state

#### Scenario: A rejected toggle reports the failure on the row

- **WHEN** the user switches a listed workspace's toggle and the presentation store cannot persist the change
- **THEN** the failure is reported on that workspace's row
- **AND** the toggle continues to show the stored state rather than the attempted one

#### Scenario: A shared toggle declares its scope before use

- **WHEN** a repository has more than one user-registered worktree listed in the settings view
- **THEN** each of those rows states that its toggle, display name, and colour are shared with the repository's other listed folders
- **AND** a workspace listed only once carries no such statement
