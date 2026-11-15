## ADDED Requirements

### Requirement: The Appearance Group Offers Theme and Scheme

The Appearance group SHALL offer two choices and a sample:

- **Theme** — one card per theme of the `theme-system` capability, in the order Forge, Blueprint, Folio, Mycelium. Each card SHALL show the theme's name, a one-line description and a preview rendered in that theme's own tokens and in the currently resolved scheme, so that a card looks like the theme it selects. Exactly one card SHALL be marked as selected, and the marking SHALL NOT rely on colour alone.
- **Scheme** — a choice among System, Light and Dark, with a line of text stating that System follows the operating system.
- **Sample** — a short fragment of document content (a heading, a sentence containing inline code and a link) rendered in the active appearance.

Both choices are choices among fixed values and SHALL persist when changed, under the *Settings Persist by One Rule* requirement. A change SHALL take effect immediately in the Settings view itself and in every other open window (see the *The Appearance Persists and Reaches Every Window* requirement in the `theme-system` capability).

Both choices SHALL be operable from the keyboard, and each card SHALL expose its theme's name and whether it is selected to assistive technology.

#### Scenario: Selecting a theme card applies it at once

- **WHEN** the user activates the Folio card
- **THEN** the Settings view and every other open window render in Folio
- **AND** the Folio card is marked as selected and no other card is

#### Scenario: A card previews its own theme

- **WHEN** the Appearance group is shown while Forge is active
- **THEN** the Blueprint card's preview is rendered with Blueprint's colours, corners and typeface
- **AND** the preview is in the scheme the window is currently resolved to

#### Scenario: Choosing a scheme applies it at once

- **WHEN** the user chooses Dark while the operating system reports light
- **THEN** the interface renders in the active theme's dark scheme
- **AND** the theme card previews are redrawn in their dark schemes

#### Scenario: The selected card is identifiable without colour

- **WHEN** the Appearance group is rendered in greyscale
- **THEN** the selected theme card can still be told apart from the others

#### Scenario: Theme cards are keyboard operable

- **WHEN** the user moves focus to a theme card with the keyboard and activates it
- **THEN** that theme is selected
- **AND** assistive technology is told the card's name and that it is selected

## MODIFIED Requirements

### Requirement: Settings Are Organised Into Groups

The Settings view of the desktop application and of the browser skin SHALL present its settings in six groups, offered in this order: **Workspaces**, **Appearance**, **Layout**, **Integrations**, **Identity** and **Desktop app**. It SHALL show exactly one group at a time, under a heading naming that group, and SHALL NOT present settings from several groups as one continuous column.

Each setting SHALL belong to exactly one group:

| Group | Settings |
|---|---|
| Workspaces | the registered-workspaces list with its per-workspace controls, the add-workspace control, and, where offered, the WSL poll interval |
| Appearance | the theme, the scheme, and their sample (see the *The Appearance Group Offers Theme and Scheme* requirement) |
| Layout | the reading width with its sample, the Commit history switch, and the BitBucket and GitHub panel positions |
| Integrations | the GitHub and BitBucket pull-request features, their desktop-only credential editors, the review skip patterns (see the *Review Skip Patterns* requirement in the `pull-request-viewer` capability), and the Claude and ChatGPT usage-quota opt-ins |
| Identity | the display name, the identities folded onto the developer, and the detected git identities |
| Desktop app | launch at login, notifications, and the embedded web server with its Tailscale options |

The Workspaces group SHALL be the default group. A host-inapplicable control within a group SHALL be omitted under *Groups With Nothing to Offer Are Omitted*; provider credential editors therefore remain in Integrations on desktop and appear in no browser group. The Appearance group applies in both hosts.

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
- **THEN** the "Skip in review" row is shown after the pull-request cards
- **AND** it is shown in no other group

#### Scenario: Credential editors stay in the same group but only on desktop

- **WHEN** the Integrations group is compared between native desktop and browser skin
- **THEN** provider credential editors occur only in the native Integrations group
- **AND** they are not moved to another browser group

#### Scenario: Appearance is offered in the browser skin

- **WHEN** the group navigation renders in the browser skin
- **THEN** it offers the Appearance group
- **AND** the theme and scheme controls in it are operable
