## MODIFIED Requirements

### Requirement: Master-Detail Browse and Screen Navigation

The interactive frontend SHALL present a Browse screen with a two-pane master-detail layout — a workspace/change tree on the left and an artifact-detail pane on the right — and SHALL provide modal Dashboard, Season, and Settings screens. Keyboard navigation SHALL move focus between the two Browse panes, switch between screens, switch between artifact tabs in the detail pane, and scroll the focused region.

#### Scenario: Browse shows tree and detail

- **WHEN** the Browse screen is active
- **THEN** the workspace/change tree and the artifact-detail pane are both shown
- **AND** keyboard focus can be moved between them

#### Scenario: Switching screens

- **WHEN** the user invokes the Dashboard, Season, or Settings screen switch
- **THEN** that screen replaces the Browse view
- **AND** returning to Browse restores the prior tree selection and detail target

#### Scenario: Selecting a change shows its artifact

- **WHEN** the user selects a change in the tree and chooses an artifact tab
- **THEN** the detail pane renders that artifact

### Requirement: Read-Only Operation

The frontend SHALL NOT modify the contents of any registered workspace. Browsing, dashboard, season, and settings views SHALL be presentation-only with respect to workspace files. The frontend MAY persist application settings to the shared configuration directory, which lives outside any registered workspace; doing so SHALL NOT constitute modifying a workspace.

#### Scenario: Browsing does not alter workspace files

- **WHEN** the user navigates workspaces, reads artifacts, and views the dashboard and season screens
- **THEN** no files inside any registered workspace are created, modified, or deleted by the frontend

#### Scenario: Settings writes target app config, not workspaces

- **WHEN** the user toggles a setting from the Settings screen
- **THEN** only the application settings file in the shared configuration directory is written
- **AND** no files inside any registered workspace are created, modified, or deleted

## ADDED Requirements

### Requirement: Settings Screen

The interactive frontend SHALL provide a Settings screen that presents the application settings the terminal frontend can act on as toggle rows, each showing its current on/off state. The first version SHALL include the gamification master switch and the Claude usage-quota opt-in. The user SHALL be able to flip each toggle, and the change SHALL be persisted immediately to the shared application settings without a separate save action. A setting changed from this screen SHALL take effect in the running frontend without requiring a restart.

#### Scenario: Settings screen lists actionable toggles

- **WHEN** the Settings screen is shown
- **THEN** a row is rendered for the gamification switch and for the Claude usage-quota opt-in
- **AND** each row shows whether that setting is currently on or off

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
