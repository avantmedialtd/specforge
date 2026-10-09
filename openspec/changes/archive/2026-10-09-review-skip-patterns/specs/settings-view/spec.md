## MODIFIED Requirements

### Requirement: Settings Are Organised Into Groups

The Settings view of the desktop application and of the browser skin SHALL present its settings in five groups, offered in this order: **Workspaces**, **Layout**, **Integrations**, **Identity** and **Desktop app**. It SHALL show exactly one group at a time, under a heading naming that group, and SHALL NOT present settings from several groups as one continuous column.

Each setting SHALL belong to exactly one group:

| Group | Settings |
|---|---|
| Workspaces | the registered-workspaces list with its per-workspace controls, the add-workspace control, and, where offered, the WSL poll interval |
| Layout | the reading width with its sample, the Commit history switch, and the BitBucket and GitHub panel positions |
| Integrations | the GitHub and BitBucket pull-request features with their credentials, the review skip patterns (see the *Review Skip Patterns* requirement in the `pull-request-viewer` capability), and the Claude and ChatGPT usage-quota opt-ins |
| Identity | the display name, the identities folded onto the developer, and the detected git identities |
| Desktop app | launch at login, notifications, and the embedded web server with its Tailscale options |

The Workspaces group SHALL be the default group.

The terminal frontend's Settings screen is not this view. It is specified by the *Terminal Settings Screen* requirement in the `terminal-ui` capability and is unaffected by this capability.

#### Scenario: Settings shows one group at a time

- **WHEN** the user opens the Settings view
- **THEN** exactly one group is shown, under a heading naming it
- **AND** no setting belonging to another group is rendered

#### Scenario: Each setting appears in exactly one group

- **WHEN** the user visits every group of the Settings view on the desktop in turn
- **THEN** each setting is presented in the group the table above assigns it
- **AND** in no other group

#### Scenario: Groups are offered in a fixed order

- **WHEN** the group navigation renders in the desktop application
- **THEN** it offers Workspaces, Layout, Integrations, Identity and Desktop app, in that order

#### Scenario: The review skip patterns sit with the pull-request integrations

- **WHEN** the GitHub pull-request integration is on and the user opens the Integrations group
- **THEN** the "Skip in review" row is shown after the pull-request cards
- **AND** it is shown in no other group
