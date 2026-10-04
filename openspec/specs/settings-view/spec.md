# settings-view Specification

## Purpose

Defines the structure of the graphical Settings view that the desktop app and the browser skin share:
- **Groups.** Settings are split into five groups (Workspaces, Layout, Integrations, Identity and Desktop app), each with fixed membership, and shown one at a time in the center pane.
- **Navigation.** The group navigation adapts to the width of the Settings view itself, not the window's.
- **Host differences.** A group with nothing to offer on the current host is omitted, as Desktop app is in the browser skin.
- **Entry points.** The sidebar row, the macOS *Settings…* item, and the re-enable paths from the Dashboard and the disabled-workspace notice all open the Workspaces group.
- **Rows.** Every setting uses one row grammar and one switch control, within a content width bounded for readable descriptions.
- **Saving.** One rule covers how every control persists its value and reports a failed write.
- **Integrations.** Opt-in integrations collapse to a single row while off.
- **Layout.** The Layout group gathers everything that decides what occupies the side panes.

Other capabilities own what each individual setting does. Each setting's capability also owns the copy that setting requires, such as a token's instructions. The terminal frontend's Settings screen is specified separately, in the `terminal-ui` capability.
## Requirements
### Requirement: Settings Are Organised Into Groups

The Settings view of the desktop application and of the browser skin SHALL present its settings in five groups, offered in this order: **Workspaces**, **Layout**, **Integrations**, **Identity** and **Desktop app**. It SHALL show exactly one group at a time, under a heading naming that group, and SHALL NOT present settings from several groups as one continuous column.

Each setting SHALL belong to exactly one group:

| Group | Settings |
|---|---|
| Workspaces | the registered-workspaces list with its per-workspace controls, the add-workspace control, and, where offered, the WSL poll interval |
| Layout | the reading width with its sample, the Commit history switch, and the BitBucket and GitHub panel positions |
| Integrations | the GitHub and BitBucket pull-request features with their credentials, and the Claude and ChatGPT usage-quota opt-ins |
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

### Requirement: Group Navigation Follows the View's Own Width

The Settings view SHALL offer navigation between its groups that indicates the group currently shown. Let $$w$$ be the Settings view's own inline size in CSS pixels. While $$w \ge 640$$, the navigation SHALL be a list of the groups beside the shown group's content. While $$w < 640$$, it SHALL be a single-line control above the content that names the current group and from which any other group can be chosen.

The width SHALL be measured on the Settings view itself, never on the window. The view shares the window with the sidebar and the commit rail and is narrowed by them, so the window's width does not tell how much room the view has.

Choosing a group SHALL show that group without reloading the view. The navigation SHALL be operable from the keyboard, SHALL expose the entry for the shown group to assistive technology as the current one, and SHALL show a visible focus indicator on each entry focused from the keyboard.

#### Scenario: A narrow view offers a single-line switcher

- **WHEN** the Settings view is rendered at the center pane's minimum width of 320 CSS pixels
- **THEN** the groups are offered by a single-line control above the content, naming the shown group
- **AND** choosing another group from it shows that group

#### Scenario: A wide view lists the groups beside the content

- **WHEN** the Settings view's own width is 960 CSS pixels
- **THEN** the groups are listed beside the shown group's content
- **AND** the entry for the shown group is marked as current

#### Scenario: The window's width does not decide

- **WHEN** the window is 1280 CSS pixels wide
- **AND** the sidebar and the commit rail leave the Settings view less than 640 CSS pixels wide
- **THEN** the groups are offered by the single-line control

#### Scenario: Only the shown group is current

- **WHEN** a group is shown
- **THEN** its navigation entry is exposed to assistive technology as the current one
- **AND** no other entry is

### Requirement: Entry Points Open the Workspaces Group

The sidebar Settings entrypoint (see the *Settings Entrypoint in Sidebar Footer* requirement in the `spec-browser` capability) and the macOS *Settings…* menu item (see the *Settings Menu Item* requirement in the `application-menu` capability) SHALL open the Settings view at the Workspaces group whenever the Settings view is not already shown.

Some navigations exist so the user can re-enable or re-add a workspace: the Dashboard's entry for a parked or unregistered row (see the *Ship Selection Opens the Archive Browser* requirement in the `dashboard` capability) and the disabled-workspace notice (see the *Cold-Load Address Resolution* requirement in the `view-routing` capability). These SHALL open the Workspaces group by naming it, so they do not depend on which group is the default.

#### Scenario: The sidebar row opens the Workspaces group

- **WHEN** the Settings view is not shown and the user activates the sidebar Settings row
- **THEN** the Settings view opens at its Workspaces group

#### Scenario: The disabled-workspace notice leads to the Workspaces group

- **WHEN** the user follows the disabled-workspace notice's way to the settings view
- **THEN** the Settings view opens at its Workspaces group, where the disabled workspace's switch is listed

### Requirement: Groups With Nothing to Offer Are Omitted

The group navigation SHALL offer a group only when at least one of its settings applies on the current host. The browser skin SHALL therefore omit the Desktop app group entirely (see the *Desktop-Only Settings Are Hidden in the Web UI* requirement in the `web-ui` capability).

Within an offered group, a setting that does not apply on the host SHALL be omitted without leaving an empty heading or placeholder. An example is the WSL poll interval outside the Windows build (see the *Configurable Poll Interval* requirement in the `wsl-workspaces` capability).

An address naming a group the current host omits SHALL show the Workspaces group instead. It SHALL replace the current history entry with the Workspaces group's address rather than adding one (see the *History Entry Discipline* requirement in the `view-routing` capability).

#### Scenario: The browser skin offers four groups

- **WHEN** the Settings view renders in the browser skin
- **THEN** the group navigation offers Workspaces, Layout, Integrations and Identity
- **AND** it offers no Desktop app group

#### Scenario: An omitted group's address falls back to Workspaces

- **WHEN** the browser skin is loaded at the address of the Desktop app group
- **THEN** the Workspaces group is shown
- **AND** the current address becomes the Workspaces group's
- **AND** no history entry is added

#### Scenario: No empty sub-section outside Windows

- **WHEN** the Workspaces group renders on macOS or Linux
- **THEN** no WSL heading, poll-interval control or placeholder is shown

### Requirement: Settings Rows

Every setting SHALL render as a **settings row**, except the registered-workspace rows, which keep the list-row grammar they share with the tree (see the *Uniform Row Grammar Across List Surfaces* requirement in the `visual-identity` capability). A settings row SHALL place the setting's title, and where the setting needs one a short description, at its leading edge, and the setting's control at its trailing edge. When the row is too narrow to hold both side by side, the control SHALL move below the text, and neither SHALL be truncated.

Every on/off setting SHALL be operated by the settings switch (see the *Settings Switch Control* requirement in the `visual-identity` capability), and this includes each registered-workspace row's enable toggle. No on/off setting SHALL render as a bare checkbox followed by a label. In a settings row, the title SHALL be the accessible name of its control, and activating the title SHALL flip an on/off setting just as activating its switch does. A registered-workspace row has no such title: its switch SHALL keep an accessible name that names the workspace and, where one applies, its shared scope (see the *Settings View* requirement in the `workspace-registry` capability). A choice among a fixed set of values, such as the reading width or a panel position, SHALL render as a row of mutually exclusive options exposed as a radio group.

The shown group's content SHALL be bounded to a readable width: on a Settings view wider than the bound, the content SHALL occupy at most $$720$$ CSS pixels of the view's inline size, and rows and their descriptions SHALL NOT stretch beyond it.

#### Scenario: An on/off setting renders as a switch row

- **WHEN** the Desktop app group renders the notifications setting
- **THEN** its row shows the title and a description at the leading edge and a settings switch at the trailing edge
- **AND** activating the title flips the switch

#### Scenario: A narrow row stacks its control below the text

- **WHEN** a settings row is rendered narrower than its text and its control need side by side
- **THEN** the control renders below the text
- **AND** neither the title nor the control is truncated

#### Scenario: Content stays readable on a wide view

- **WHEN** the Settings view is 1600 CSS pixels wide
- **THEN** the shown group's content occupies at most 720 CSS pixels of it

#### Scenario: The workspace enable toggle is the same switch

- **WHEN** a registered-workspace row is rendered in the Workspaces group
- **THEN** its enable toggle is a settings switch

### Requirement: Settings Persist by One Rule

Each kind of control SHALL persist its value by one rule, the same in every group and on both hosts:

- An on/off switch, or a choice among fixed values, SHALL persist when it is changed.
- A free-text or numeric field holding a setting's value SHALL persist when its edit is committed, either by pressing Enter or by moving focus out of the field. It SHALL NOT persist on each keystroke. Pressing Escape in such a field SHALL abandon the edit, restore the stored value without persisting, and SHALL NOT also dismiss the Settings view.
- A credential SHALL persist only through the explicit save action beside it, and SHALL NOT persist when focus leaves its field. A credential is a token, or the BitBucket username submitted together with its token.

A committed value the setting does not accept SHALL NOT be persisted. Examples are a web-server port $$p$$ outside $$1 \le p \le 65535$$, a poll interval below one second, and a non-numeric entry in a numeric field. The rejection SHALL be reported on the field, and the stored value SHALL remain in effect.

A write that the backend rejects or cannot persist SHALL be reported on the control the user operated, and that control SHALL continue to show the stored value rather than the attempted one. Reporting the failure only to a developer console does not satisfy this. The desktop and browser frontends have no console the user can see, so a rejected write would otherwise be indistinguishable from a control that does nothing.

#### Scenario: Typing a port persists once

- **WHEN** the user types 4317 into the web-server port field and presses Enter
- **THEN** exactly one write is made, persisting 4317
- **AND** no write is made while the digits are being typed

#### Scenario: Escape abandons a field edit

- **WHEN** the user edits the display name in the Identity group and presses Escape
- **THEN** the field shows the stored display name
- **AND** nothing is persisted
- **AND** the Settings view remains open

#### Scenario: An out-of-range port is refused on the field

- **WHEN** the user commits 70000 as the web-server port
- **THEN** nothing is persisted
- **AND** the field reports that the port must be between 1 and 65535
- **AND** the stored port remains in effect

#### Scenario: A credential is not sent when focus leaves its field

- **WHEN** the user types a GitHub token and moves focus out of the field without using the save action
- **THEN** no write is made

#### Scenario: A failed switch reports on the switch

- **WHEN** the user flips a settings switch and the write fails
- **THEN** the failure is reported beside that switch
- **AND** the switch shows the stored state

### Requirement: Opt-In Integrations Collapse While Off

Each opt-in integration in the Integrations group SHALL render as a card whose content depends on whether it is on. The integrations are GitHub pull requests, BitBucket pull requests, Claude usage and ChatGPT usage.

While an integration is off, its card SHALL present only:

- its name;
- a description stating what turning it on reads and where it sends requests;
- its switch.

It SHALL NOT present credential fields, setup instructions or panel placement. When a credential is stored for an integration that is off, the card SHALL say so and SHALL offer to remove the stored credential without turning the integration on.

While an integration that takes a credential is on, its card SHALL additionally present:

- its credential fields with their save action;
- the setup instructions its capability requires (see the *Credentials Are Stored Write-Only* requirement in the `bitbucket-pull-requests` capability and the *The GitHub Token Is Stored Write-Only* requirement in the `github-pull-requests` capability).

The setup instructions SHALL be expanded while no credential is stored. Once one is, they SHALL be collapsed and remain available on request.

A pull-request integration that is on SHALL state which slot its panel occupies, and SHALL offer a way to reach the Layout group, where that slot is chosen.

The Claude and ChatGPT usage opt-ins take no credential in Settings. Their cards SHALL present only their name, description and switch, whether on or off.

#### Scenario: An integration that is off is a single row

- **WHEN** the GitHub pull-request integration is off and no token is stored
- **THEN** its card shows its name, its description and its switch
- **AND** no token field, setup instructions or panel position is shown

#### Scenario: Turning an integration on reveals its credentials

- **WHEN** the user turns the BitBucket pull-request integration on while no token is stored
- **THEN** its card shows the username and token fields with a save action
- **AND** the setup instructions are shown expanded

#### Scenario: Instructions fold away once a credential is stored

- **WHEN** the GitHub pull-request integration is on and a token is stored
- **THEN** its setup instructions are collapsed
- **AND** the user can expand them

#### Scenario: A stored token can be removed while off

- **WHEN** the GitHub pull-request integration is off and a token is stored
- **THEN** its card states that a token is stored and offers to remove it
- **AND** removing it clears the stored token without turning the integration on

#### Scenario: An enabled pull-request card points to its panel's slot

- **WHEN** the GitHub pull-request integration is on
- **THEN** its card names the slot the GitHub panel occupies
- **AND** it offers a way to the Layout group

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

