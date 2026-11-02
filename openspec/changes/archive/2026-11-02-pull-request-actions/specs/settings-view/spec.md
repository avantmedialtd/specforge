## ADDED Requirements

### Requirement: Provider Credential Editing Is Desktop-Only

GitHub and BitBucket credential fields, active/stored source state, save actions, clear actions and legacy re-save prompts SHALL render only in the native desktop Settings view. The browser skin SHALL keep the integrations' enabled switches, descriptions and links to applicable non-secret layout settings, but SHALL render no credential value, source state, field or mutation control. It SHALL state that provider credentials are managed in the desktop app when guidance is needed. The terminal frontend SHALL retain only its existing provider enabled toggles and SHALL expose no credential source, field, save or clear operation.

This host boundary SHALL apply whether the active credential comes from the environment, the operating-system credential store or legacy plaintext settings. Hiding the controls SHALL be backed by absent web-dispatch operations and native-only API guards (see the *Command Transport Mirrors the In-Process Command Surface* requirement in the `web-ui` capability), not by CSS alone.

#### Scenario: Browser integrations have no credential editor

- **WHEN** the Integrations group renders in the browser skin with GitHub enabled
- **THEN** it shows the enabled switch and applicable non-secret integration information
- **AND** it shows no token field, save action, clear action or legacy re-save control

#### Scenario: Native Settings can replace a legacy credential

- **WHEN** native Settings reports that a GitHub or BitBucket credential comes from legacy plaintext settings
- **THEN** it offers an explicit re-save path to the operating-system credential store
- **AND** it never fills the credential field with the old secret

#### Scenario: Hiding a control is not the boundary

- **WHEN** browser code attempts to call a provider credential setter directly
- **THEN** the frontend refuses the native-only call and the web server has no matching dispatch operation

#### Scenario: Terminal settings cannot edit a credential

- **WHEN** the terminal Settings screen shows the GitHub and BitBucket integrations
- **THEN** it offers their enabled toggles but no credential value, source, field, save action or clear action

## MODIFIED Requirements

### Requirement: Settings Are Organised Into Groups

The Settings view of the desktop application and of the browser skin SHALL present its settings in five groups, offered in this order: **Workspaces**, **Layout**, **Integrations**, **Identity** and **Desktop app**. It SHALL show exactly one group at a time, under a heading naming that group, and SHALL NOT present settings from several groups as one continuous column.

Each setting SHALL belong to exactly one group:

| Group | Settings |
|---|---|
| Workspaces | the registered-workspaces list with its per-workspace controls, the add-workspace control, and, where offered, the WSL poll interval |
| Layout | the reading width with its sample, the Commit history switch, and the BitBucket and GitHub panel positions |
| Integrations | the GitHub and BitBucket pull-request features, their desktop-only credential editors, the review skip patterns (see the *Review Skip Patterns* requirement in the `pull-request-viewer` capability), and the Claude and ChatGPT usage-quota opt-ins |
| Identity | the display name, the identities folded onto the developer, and the detected git identities |
| Desktop app | launch at login, notifications, and the embedded web server with its Tailscale options |

The Workspaces group SHALL be the default group. A host-inapplicable control within a group SHALL be omitted under *Groups With Nothing to Offer Are Omitted*; provider credential editors therefore remain in Integrations on desktop and appear in no browser group.

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

#### Scenario: Credential editors stay in the same group but only on desktop

- **WHEN** the Integrations group is compared between native desktop and browser skin
- **THEN** provider credential editors occur only in the native Integrations group
- **AND** they are not moved to another browser group

### Requirement: Settings Persist by One Rule

Each kind of control SHALL persist its value by one rule in every group where that control applies:

- An on/off switch, or a choice among fixed values, SHALL persist when it is changed.
- A free-text or numeric field holding a setting's value SHALL persist when its edit is committed, either by pressing Enter or by moving focus out of the field. It SHALL NOT persist on each keystroke. Pressing Escape in such a field SHALL abandon the edit, restore the stored value without persisting, and SHALL NOT also dismiss the Settings view.
- A provider credential SHALL persist only through the explicit save action beside it in native Settings, and SHALL NOT persist when focus leaves its field. A credential is a GitHub token, or the BitBucket username and token submitted as one pair. Its successful save SHALL use the operating-system credential store as the provider capability requires. No provider credential control SHALL apply in the browser skin.

A committed value the setting does not accept SHALL NOT be persisted. Examples are a web-server port $$p$$ outside $$1 \le p \le 65535$$, a poll interval below one second, and a non-numeric entry in a numeric field. The rejection SHALL be reported on the field, and the stored value SHALL remain in effect.

A write that the backend rejects or cannot persist SHALL be reported on the control the user operated, and that control SHALL continue to show the stored state rather than the attempted one. Reporting the failure only to a developer console does not satisfy this. A failed operating-system credential save, replacement or clear SHALL not fall back to plaintext settings or leave a half-applied source state; the provider capability's rollback-safe transaction SHALL preserve the prior resolved and stored state. The desktop and browser frontends have no console the user can see, so a rejected applicable write would otherwise be indistinguishable from a control that does nothing.

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

- **WHEN** the user types a GitHub token in native Settings and moves focus out of the field without using the save action
- **THEN** no credential-store or settings write is made

#### Scenario: A secure-store failure does not write plaintext

- **WHEN** the operating-system credential store rejects a submitted provider credential
- **THEN** the failure is reported beside that credential control
- **AND** the attempted credential is absent from application settings and is not treated as stored

#### Scenario: A failed credential clear keeps the prior state

- **WHEN** either half of clearing a stored provider credential fails
- **THEN** the failure is reported beside the clear action
- **AND** the control continues to report the previously resolved and stored source state

#### Scenario: A failed switch reports on the switch

- **WHEN** the user flips a settings switch and the write fails
- **THEN** the failure is reported beside that switch
- **AND** the switch shows the stored state

### Requirement: Opt-In Integrations Collapse While Off

Each opt-in integration in the Integrations group SHALL render as a card whose content depends on whether it is on and on the current host. The integrations are GitHub pull requests, BitBucket pull requests, Claude usage and ChatGPT usage.

While an integration is off, its card SHALL present only:

- its name;
- a description stating what turning it on reads and where it sends requests;
- its switch; and
- in native Settings only, a non-secret stored-credential state and clear action when one exists. An active environment credential SHALL NOT hide an underlying secure or legacy entry from this stored state, and clearing that entry SHALL NOT claim to clear the environment.

It SHALL NOT present credential fields, setup instructions or panel placement while off. Clearing a stored provider credential in native Settings SHALL NOT turn the integration on. The browser skin SHALL present no credential state or clear action.

While a provider integration is on, its native card SHALL additionally present:

- empty credential fields with their explicit save action;
- the non-secret active source (`environment`, `secure`, `legacy` or `none`) and stored source (`secure`, `legacy` or `none`);
- the setup and review-scope instructions its capability requires (see the *Credentials Are Stored Write-Only* requirement in the `bitbucket-pull-requests` capability and the *The GitHub Token Is Stored Write-Only* requirement in the `github-pull-requests` capability); and
- for a legacy source, an explicit prompt to re-save securely before review actions become available.

The native setup instructions SHALL be expanded while no credential is available. Once a secure or environment credential is available, they SHALL be collapsed and remain available on request. The browser card SHALL instead omit all credential controls and SHALL state that credentials are managed in the desktop app when no active credential can power the integration.

A pull-request integration that is on SHALL state which slot its panel occupies, and SHALL offer a way to reach the Layout group, where that slot is chosen.

The Claude and ChatGPT usage opt-ins take no credential in Settings. Their cards SHALL present only their name, description and switch, whether on or off.

#### Scenario: An integration that is off is a single row

- **WHEN** the GitHub pull-request integration is off and no token is stored
- **THEN** its card shows its name, its description and its switch
- **AND** no token field, setup instructions or panel position is shown

#### Scenario: Turning an integration on reveals its credentials

- **WHEN** the user turns the BitBucket pull-request integration on in native Settings while no credential is available
- **THEN** its card shows empty username and token fields with a save action
- **AND** the setup instructions are shown expanded

#### Scenario: Turning an integration on in the browser reveals no credential control

- **WHEN** the user turns the BitBucket pull-request integration on in the browser skin
- **THEN** its card shows no username field, token field, save action or clear action
- **AND** it states that credentials are managed in the desktop app when one is needed

#### Scenario: Instructions fold away once a credential is stored

- **WHEN** the GitHub pull-request integration is on in native Settings and a secure token is stored
- **THEN** its setup instructions are collapsed
- **AND** the user can expand them

#### Scenario: A stored token can be removed while off

- **WHEN** the GitHub pull-request integration is off in native Settings and a secure or legacy token is stored
- **THEN** its card states the non-secret storage source and offers to remove it
- **AND** removing it clears stored secure and legacy values without turning the integration on

#### Scenario: Environment and stored sources are shown separately

- **WHEN** an environment credential is active while a secure or legacy credential is also stored
- **THEN** native Settings identifies the environment as active and the stored source separately
- **AND** its clear action removes only the stored credential and leaves the environment source active

#### Scenario: A legacy credential asks to be re-saved

- **WHEN** an enabled native provider card reports a legacy credential source
- **THEN** it explains that reads continue but reviews require an explicit secure re-save
- **AND** it does not display the legacy value

#### Scenario: An enabled pull-request card points to its panel's slot

- **WHEN** the GitHub pull-request integration is on
- **THEN** its card names the slot the GitHub panel occupies
- **AND** it offers a way to the Layout group

### Requirement: The Settings File Is Saved Atomically and in Order

Every write of the settings file SHALL be atomic. The new contents SHALL be written to a uniquely named temporary file in the settings file's directory, flushed to disk, and renamed over the settings file, so that a crash, a failed write or a concurrent writer can never leave a truncated or partly written settings file. A write that fails SHALL leave the previous settings file intact and SHALL leave no temporary file behind. The failure SHALL be returned to the caller and reported as the *Settings Persist by One Rule* requirement describes. It SHALL NOT be swallowed.

Except when the instance's startup load already forbids every save under *A Damaged Settings File Is Never Overwritten by Defaults*, every settings mutation SHALL take a process-local write lock and an inter-process sidecar lock, then re-read the current parseable settings file, or use defaults when the file is absent. It SHALL apply only the requested mutation to that fresh state, atomically replace the file, and only then publish the resulting state in memory. This order SHALL preserve unrelated changes made by another instance rather than serialising an older whole-file snapshot over them. If a present file cannot be read or parsed, the mutation SHALL fail without overwriting or moving it. Within one running instance, saves SHALL still reach the file in the order their changes were made. Reading the current in-memory settings SHALL NOT wait for a save's file I/O to finish.

A provider credential save, replacement or clear SHALL hold that same inter-process lock across its staged secure-store work, fresh settings read, settings activation/removal and rollback or cleanup. It SHALL durably identify the active secure entry and any in-progress transaction using non-secret metadata. Before a later settings mutation or durable provider-credential resolution proceeds, it SHALL recover interrupted transaction state under the same lock: an entry staged before the settings commit remains ineligible and is rolled back, while a committed replacement or clear remains authoritative and cleanup of a retired entry is completed or retained for retry. Recovery metadata SHALL contain no credential. Consequently, a stale desktop or standalone-server instance SHALL NOT resurrect a removed legacy plaintext credential or erase secure-source or recovery metadata when it later writes an unrelated setting.

#### Scenario: A failed save leaves the previous file intact

- **WHEN** a settings file holding configured GitHub credential state exists
- **AND** a later save fails before the rename completes
- **THEN** the settings file still holds its previous contents
- **AND** no temporary file is left beside it
- **AND** the control that triggered the save reports the failure

#### Scenario: Concurrent changes both reach disk

- **WHEN** two settings are changed concurrently in the same instance
- **THEN** after both changes return, the settings file on disk holds both new values
- **AND** the settings file equals the settings the instance holds in memory

#### Scenario: Separate instances preserve unrelated changes

- **WHEN** one instance saves a credential securely and removes its legacy plaintext while an older instance later changes a panel position
- **THEN** the older instance re-reads the saved credential metadata and missing plaintext under the inter-process lock before applying the position
- **AND** the resulting file contains both changes and no resurrected plaintext credential

#### Scenario: An interrupted credential stage grants no authority

- **WHEN** a process exits after staging a replacement credential but before the settings commit activates it
- **THEN** the next durable credential resolution recovers the transaction under the inter-process lock
- **AND** the staged credential is never returned as active or accepted for a review write

#### Scenario: A committed credential change survives interrupted cleanup

- **WHEN** settings commits a credential replacement or clear and the process exits before its retired secure entry is removed
- **THEN** recovery preserves the committed active or cleared state
- **AND** it completes the retired-entry cleanup or retains non-secret cleanup state for a later retry without making the retired entry eligible

#### Scenario: A newly damaged file is not overwritten

- **WHEN** an instance loaded valid settings but finds the settings file unreadable or unparsable after acquiring the inter-process lock for a later mutation
- **THEN** that mutation fails and leaves the file untouched
- **AND** the failure is reported on the control that requested it

#### Scenario: Reading settings does not wait on a save

- **WHEN** a settings save is in progress
- **THEN** a read of the current in-memory settings returns without waiting for the save's file I/O
