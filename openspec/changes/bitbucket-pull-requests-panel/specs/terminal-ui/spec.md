## MODIFIED Requirements

### Requirement: Terminal Settings Screen

The interactive frontend SHALL provide a Settings screen that presents the application settings the terminal frontend can act on. The screen SHALL present a set of toggle rows — each showing its current on/off state — an Appearance control for choosing the active colour scheme, and a Workspaces section listing the user-registered workspaces with controls to add, remove, rename, recolor, and enable/disable them. The toggles SHALL include the Claude usage-quota opt-in, the ChatGPT usage-quota opt-in, and the BitBucket pull-requests opt-in, and SHALL NOT include any control that hides the progress surfaces. The user SHALL be able to flip each toggle, and the change SHALL be persisted immediately to the shared application settings without a separate save action. The Appearance control SHALL let the user choose among the available colour schemes; the choice SHALL be persisted to the terminal frontend's own configuration and SHALL take effect immediately. A setting changed from this screen SHALL take effect in the running frontend without requiring a restart, where the terminal frontend acts on that setting; the BitBucket pull-requests toggle writes the shared setting but the terminal frontend neither polls nor renders pull requests (see the *The Terminal Frontend Does Not Render the Panel* requirement in the `bitbucket-pull-requests` capability). The behaviour of the Workspaces section is specified by the Workspace Management from the Terminal requirement.

#### Scenario: Settings screen lists actionable toggles

- **WHEN** the Settings screen is shown
- **THEN** a row is rendered for the Claude usage-quota opt-in, for the ChatGPT usage-quota opt-in, and for the BitBucket pull-requests opt-in
- **AND** each row shows whether that setting is currently on or off

#### Scenario: No toggle hides the progress surfaces

- **WHEN** the Settings screen is shown
- **THEN** no row offering to disable the Dashboard, Garden, or heatmap content is rendered

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

#### Scenario: Toggling the quota opt-in updates the title-bar gauge

- **WHEN** the user disables the Claude usage-quota opt-in on the Settings screen
- **THEN** the title-bar quota gauge is cleared without restarting the frontend
- **AND** re-enabling it shows the gauge again once the quota poller next refreshes

#### Scenario: Toggling the BitBucket opt-in writes the shared setting only

- **WHEN** the user flips the BitBucket pull-requests opt-in on the Settings screen
- **THEN** `bitbucket.enabled` is written to the shared application settings
- **AND** the terminal frontend makes no request to BitBucket and renders no pull-request list
