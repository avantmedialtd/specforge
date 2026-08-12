# terminal-ui Specification Delta

## ADDED Requirements

### Requirement: Terminal Dashboard Notes Disabled Workspaces

The terminal frontend's Dashboard screen SHALL note, whenever at least one
top-level row is disabled, that its totals include disabled workspaces. This
satisfies, for the terminal client, the *Dashboard Unaffected by Workspace
Disable* requirement in the `dashboard` capability: the Dashboard remains the
unfiltered record while the Browse tree hides exactly those rows, so without the
note a terminal user reads the difference between the two as a counting bug. The
note SHALL be omitted entirely when no row is disabled, and SHALL be placed with
the totals it qualifies rather than out of view.

The number the note reports SHALL be the count of *top-level rows* the Browse
tree drops — not the count of registered entries carrying the disabled flag. The
flag is keyed per top-level row, so a repository registered at several worktrees
is one row and SHALL be counted once, however many of its worktrees are
registered. Each non-git (flat) workspace is its own top-level row and SHALL be
counted individually.

Disabling a workspace SHALL NOT change any figure the Dashboard reports; the
note explains the totals, it does not adjust them.

#### Scenario: No note when nothing is disabled

- **WHEN** the Dashboard screen is shown and no top-level row is disabled
- **THEN** no disabled-workspace note is rendered

#### Scenario: Note appears when a workspace is parked

- **WHEN** one top-level row is disabled and the Dashboard screen is shown
- **THEN** the Dashboard notes that its totals include one disabled workspace
- **AND** parking a second row raises the number the note reports to two

#### Scenario: A repository registered at two worktrees counts once

- **WHEN** one repository is user-registered at two of its worktrees, so the Settings list shows two entries for it
- **AND** that repository is disabled
- **THEN** the Browse tree loses one top-level row
- **AND** the Dashboard's note reports one disabled workspace, not two

#### Scenario: The note explains the totals rather than changing them

- **WHEN** a workspace holding active changes is disabled
- **THEN** the Dashboard's summary totals are unchanged
- **AND** the Browse tree no longer reaches that workspace's changes
- **AND** the note accounts for the difference

## MODIFIED Requirements

### Requirement: Read-Only Operation

The frontend SHALL NOT modify the contents of any registered workspace. Browsing, dashboard, season, and settings views SHALL be presentation-only with respect to workspace files. The frontend MAY persist application configuration to the shared configuration directory — including application settings, the registered-workspace list, and per-workspace presentation overrides (display name, palette colour, and disabled state) — which lives outside any registered workspace; doing so SHALL NOT constitute modifying a workspace. Registering, unregistering, or disabling a workspace changes only the application's record of which folders to observe and how to present them; it SHALL NOT create, modify, or delete any file inside the affected folder.

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

#### Scenario: Disabling a workspace does not write inside the workspace

- **WHEN** the user disables or re-enables a workspace from the Settings screen
- **THEN** only the presentation store in the application's configuration directory is written
- **AND** no files inside that workspace are created, modified, or deleted

### Requirement: Settings Screen

The interactive frontend SHALL provide a Settings screen that presents the application settings the terminal frontend can act on. The screen SHALL present a set of toggle rows — each showing its current on/off state — an Appearance control for choosing the active colour scheme, and a Workspaces section listing the user-registered workspaces with controls to add, remove, rename, recolor, and enable/disable them. The first version's toggles SHALL include the gamification master switch and the Claude usage-quota opt-in. The user SHALL be able to flip each toggle, and the change SHALL be persisted immediately to the shared application settings without a separate save action. The Appearance control SHALL let the user choose among the available colour schemes; the choice SHALL be persisted to the terminal frontend's own configuration and SHALL take effect immediately. A setting changed from this screen SHALL take effect in the running frontend without requiring a restart. The behaviour of the Workspaces section is specified by the Workspace Management from the Terminal requirement.

#### Scenario: Settings screen lists actionable toggles

- **WHEN** the Settings screen is shown
- **THEN** a row is rendered for the gamification switch and for the Claude usage-quota opt-in
- **AND** each row shows whether that setting is currently on or off

#### Scenario: Settings screen offers a colour scheme control

- **WHEN** the Settings screen is shown
- **THEN** an Appearance control lists the available colour schemes and indicates the active one

#### Scenario: Settings screen lists registered workspaces

- **WHEN** the Settings screen is shown
- **THEN** a Workspaces section lists every user-registered workspace with its name and folder path
- **AND** an add-workspace control is shown

#### Scenario: Toggling a setting persists immediately

- **WHEN** the user flips a toggle on the Settings screen
- **THEN** the new value is written to the shared application settings without a separate save action
- **AND** the value is still in effect when the frontend is restarted

#### Scenario: Choosing a colour scheme persists across restart

- **WHEN** the user selects a colour scheme from the Appearance control
- **THEN** the interface is redrawn in that scheme immediately
- **AND** the same scheme is active when the frontend is restarted

#### Scenario: Toggling gamification updates the gamified surfaces

- **WHEN** the user toggles the gamification switch on the Settings screen
- **THEN** the gamified surfaces (Dashboard, Season, Garden) reflect the new state without restarting the frontend

#### Scenario: Toggling the quota opt-in updates the title-bar gauge

- **WHEN** the user disables the Claude usage-quota opt-in on the Settings screen
- **THEN** the title-bar quota gauge is cleared without restarting the frontend
- **AND** re-enabling it shows the gauge again once the quota poller next refreshes

### Requirement: Workspace Management from the Terminal

The interactive frontend SHALL allow the user to manage the registered-workspace set from the Settings screen: add a workspace by path, remove a user-registered workspace, set a workspace's display name and palette colour, and disable or re-enable a workspace. These operations SHALL go through the shared application service so the registry, the filesystem watcher, and the presentation store stay consistent, and their effects SHALL appear in the running frontend without a restart. The Settings workspace list SHALL present the user-registered workspaces only; auto-discovered worktrees SHALL NOT appear as manageable rows.

Every operation available on the focused workspace row SHALL be advertised in the frontend's key hints, so no control — the disable toggle included — is reachable only by prior knowledge. Because disabling removes a workspace's top-level row from the Browse tree, the Settings list SHALL remain its home: a disabled workspace SHALL stay listed there, SHALL be visibly marked as disabled, and SHALL be re-enabled from that same row, so the terminal frontend is a complete surface for the operation and does not require the desktop shell or the web UI to undo it.

Disabling SHALL be applied immediately without a confirmation step — it is reversible from the row that performed it — and SHALL be keyed the same way the shared service keys presentation overrides, so sibling worktrees of one repository share a single disabled state.

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

#### Scenario: Disable a workspace from the Settings screen

- **WHEN** the user invokes the disable control on a workspace row
- **THEN** the disabled state is persisted to the presentation store without a confirmation step
- **AND** that workspace's top-level row leaves the Browse tree without a restart
- **AND** the row stays in the Settings list, marked as disabled

#### Scenario: Re-enable a workspace from the same row

- **WHEN** the user invokes the same control on a workspace row that is already disabled
- **THEN** the workspace is re-enabled and its top-level row returns to the Browse tree without a restart
- **AND** the disabled marker is cleared from its Settings row

#### Scenario: The disable control is advertised

- **WHEN** the cursor is on a workspace row in the Settings screen
- **THEN** the key hints name the control that disables and re-enables it, alongside the add, remove, rename and colour controls

#### Scenario: Disabling does not stop the workspace being tracked

- **WHEN** a workspace is disabled
- **THEN** its filesystem watcher keeps running and its changes keep reaching the Dashboard
- **AND** only its presence in the Browse tree is withdrawn
