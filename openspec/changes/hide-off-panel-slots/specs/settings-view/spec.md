## MODIFIED Requirements

### Requirement: The Layout Group Gathers the Side Panes' Occupants

The Layout group SHALL present together:

- the reading width with its sample (see the *Reading Width Is a Selectable Preference* requirement in the `document-width` capability);
- the Commit history switch (see the *Commit History Can Be Turned Off* requirement in the `commit-graph` capability);
- one panel-position choice per pull-request provider that is on, BitBucket then GitHub, matching the order in which panels sharing a slot stack (see the *Panel Position Is a Persisted Setting* requirement in the `bitbucket-pull-requests` capability and the *GitHub Panel Position Is a Persisted Setting* requirement in the `github-pull-requests` capability).

The panel positions SHALL NOT also be presented in the Integrations group.

A pull-request provider that is off SHALL have no panel-position choice in the Layout group. Nothing SHALL stand in for the absent choice: no note, no link, and no loading or error placeholder while the provider's configuration is read. The way to a provider's slot is its enabled card in the Integrations group (see the *Opt-In Integrations Collapse While Off* requirement). A slot chosen while the provider was on SHALL stay persisted while it is off, and SHALL be where its panel lands when the provider is turned on again.

A panel position changed in another window or another connected client SHALL be reflected in an open Layout group without reopening it.

#### Scenario: The layout settings are presented together

- **WHEN** the BitBucket and GitHub pull-request integrations are both on and the user opens the Layout group
- **THEN** the reading width, the Commit history switch and both panel-position choices are shown in that group

#### Scenario: Panel position is chosen only in Layout

- **WHEN** the user opens the Integrations group with the GitHub pull-request integration on
- **THEN** no panel-position choice is shown there

#### Scenario: The position of an off provider's panel

- **WHEN** the BitBucket pull-request integration is off, the GitHub one is on, and the user opens the Layout group
- **THEN** no BitBucket panel-position choice is shown, and nothing stands in for it
- **AND** the GitHub panel-position choice is shown

#### Scenario: A move made elsewhere is reflected

- **WHEN** the Layout group is open and the GitHub panel's position is changed from another window
- **THEN** the open Layout group shows the new position without being reopened

#### Scenario: No pull-request integration is on

- **WHEN** both pull-request integrations are off and the user opens the Layout group
- **THEN** its Side panes section shows the Commit history switch and no panel-position choice

#### Scenario: A slot survives turning its provider off and on

- **WHEN** the GitHub panel is placed at Rail top, the GitHub integration is turned off, and later turned on again
- **THEN** the GitHub panel-position choice shows Rail top
- **AND** the panel appears at Rail top
