## MODIFIED Requirements

### Requirement: Settings Are Organised Into Groups

The Settings view of the desktop application and of the browser skin SHALL present its settings in six groups, offered in this order: **Workspaces**, **Appearance**, **Layout**, **Integrations**, **Identity** and **Desktop app**. It SHALL show exactly one group at a time, under a heading naming that group, and SHALL NOT present settings from several groups as one continuous column.

Each setting SHALL belong to exactly one group:

| Group | Settings |
|---|---|
| Workspaces | the registered-workspaces list with its per-workspace controls, the add-workspace control, and, where offered, the WSL poll interval |
| Appearance | the theme, the scheme, and their sample (see the *The Appearance Group Offers Theme and Scheme* requirement) |
| Layout | the reading width with its sample, the Commit history switch, and the BitBucket and GitHub panel positions |
| Integrations | the GitHub and BitBucket pull-request features, their desktop-only credential editors, the two automatic PR loading preferences (see the *Pull-Request Loading Preferences* requirement in the `pr-content-loading` capability), the review skip patterns (see the *Review Skip Patterns* requirement in the `pull-request-viewer` capability), and the Claude and ChatGPT usage-quota opt-ins |
| Identity | the display name, the identities folded onto the developer, and the detected git identities |
| Desktop app | launch at login, notifications, and the embedded web server with its Tailscale options |

While either PR provider is enabled, Integrations SHALL show **Automatically show images** and **Automatically load large diffs** after the provider cards and before “Skip in review”. The image row SHALL explain that it covers changed-file comparisons and supported images in descriptions/comments; that embedded-image hosts can observe read timing and the IP of the machine running SpecForge; that embedded-image requests carry no credentials while changed-file previews use the configured provider API; and that protected/unsupported embedded images remain links. The large-diff row SHALL explain near-viewport withheld-file loading, possible provider API requests and unchanged preview/rate limits. Both rows SHALL use the existing switch, immediate persistence and row-local failure behaviour. With both providers off, their rows SHALL be omitted without clearing their saved choices.

The Workspaces group SHALL be the default group. A host-inapplicable control within a group SHALL be omitted under *Groups With Nothing to Offer Are Omitted*; provider credential editors therefore remain in Integrations on desktop and appear in no browser group. The Appearance group and both PR loading preferences SHALL apply in both hosts.

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
- **THEN** it offers Workspaces, Appearance, Layout, Integrations, Identity and Desktop app, in that order

#### Scenario: The review skip patterns sit with the pull-request integrations

- **WHEN** the GitHub pull-request integration is on and the user opens the Integrations group
- **THEN** the "Skip in review" row is shown after the pull-request cards and loading preferences
- **AND** it is shown in no other group

#### Scenario: Credential editors stay in the same group but only on desktop

- **WHEN** the Integrations group is compared between native desktop and browser skin
- **THEN** provider credential editors occur only in the native Integrations group
- **AND** they are not moved to another browser group

#### Scenario: Appearance is offered in the browser skin

- **WHEN** the group navigation renders in the browser skin
- **THEN** it offers the Appearance group
- **AND** the theme and scheme controls in it are operable

#### Scenario: Loading switches sit together on both rich hosts

- **WHEN** either PR provider is enabled and Integrations is opened on desktop or browser
- **THEN** both loading switches appear after the provider cards and before review skip patterns
- **AND** no new Settings group or per-provider duplicate is offered

#### Scenario: Image loading discloses its external request

- **WHEN** the image preference is presented in a served browser client
- **THEN** its description identifies embedded-host timing/IP exposure and credentialless requests
- **AND** it makes clear that the IP belongs to the machine running SpecForge rather than necessarily the browser device

#### Scenario: Hidden controls retain saved preferences

- **WHEN** both providers are disabled after a loading preference was enabled
- **THEN** the loading rows disappear without clearing that preference
- **AND** re-enabling either provider shows its stored choice
