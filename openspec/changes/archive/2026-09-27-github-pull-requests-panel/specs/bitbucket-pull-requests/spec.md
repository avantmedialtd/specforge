## MODIFIED Requirements

### Requirement: The Snapshot Is Announced on the Cache Stream

A changed snapshot SHALL be announced by a `bitbucket-pull-requests-updated` event carrying no payload, derived from a cache-event variant so it reaches the desktop forwarder and the browser skin's event stream through the one shared envelope mapping. Frontends SHALL re-read the snapshot through `get_bitbucket_pull_requests` on receipt. An unchanged snapshot SHALL NOT be announced. The event and the getter SHALL name BitBucket, so neither is mistaken for the GitHub panel's (see the *The GitHub Snapshot Is Announced on the Cache Stream* requirement in the `github-pull-requests` capability), and a BitBucket refresh SHALL NOT cause the GitHub panel to re-read its snapshot. The provider-less names `pull-requests-updated` and `get_my_pull_requests` SHALL NOT be served on any transport.

#### Scenario: A refresh that changes nothing is silent

- **WHEN** a refresh yields a snapshot equal to the cached one
- **THEN** no event is emitted

#### Scenario: The browser skin receives the announcement

- **WHEN** the snapshot changes while the browser skin is connected
- **THEN** the browser skin receives `bitbucket-pull-requests-updated` over its event stream and re-reads the snapshot
- **AND** the GitHub panel, if rendered, does not re-read its own

#### Scenario: The provider-less getter is retired

- **WHEN** `get_my_pull_requests` is sent to the web transport's dispatch surface
- **THEN** it is reported as an unknown command

### Requirement: Panel Position Is a Persisted Setting

The panel's position SHALL be a persisted `bitbucket.panelPosition` setting drawn from exactly four values — `left-top`, `left-bottom`, `right-top`, `right-bottom` — with the default `left-bottom`. It SHALL be chosen in Settings, in both the desktop application and the browser skin, SHALL survive a restart, and SHALL NOT be stored per window. A change SHALL be announced by a dedicated `pull-request-panel-moved` event carrying `{ provider: "bitbucket", position }` on both the desktop and the browser event transports, so windows already open re-seat the BitBucket panel without reopening and leave the GitHub panel, if rendered, where it is (see the *GitHub Panel Position Is a Persisted Setting* requirement in the `github-pull-requests` capability). A stored value the running version does not recognise SHALL load as the default and SHALL NOT fail the load of the settings as a whole.

#### Scenario: A chosen position survives a restart

- **WHEN** the user selects `right-top` and restarts the application
- **THEN** the panel renders above the commit rail from the first frame

#### Scenario: An open window follows the change

- **WHEN** the position is changed in Settings while another window or a connected browser skin is open
- **THEN** that window re-seats the panel at the new position without being reopened
- **AND** a GitHub panel, if rendered, stays in its own slot

#### Scenario: An unknown position degrades to the default

- **WHEN** the settings file carries a panel position the running version does not recognise
- **THEN** the panel renders at `left-bottom`
- **AND** every other setting in the file loads intact

### Requirement: Opening a Pull Request

Activating a row SHALL open the pull request's BitBucket web page. In the desktop application this SHALL go through a dedicated `open_pull_request` command whose service layer accepts only a URL that is the web URL of a row in the current BitBucket snapshot or in either list of the current GitHub snapshot (see the *Opening a GitHub Pull Request* requirement in the `github-pull-requests` capability) and refuses any other value, and which hands the accepted URL to the platform opener; the frontend SHALL NOT thereby gain a general open-URL capability. The command SHALL be absent from the web transport's dispatch surface, consistent with the *Link Handling in the Browser Skin* requirement in the `web-ui` capability. In the browser skin a row SHALL be a link that opens in a new opener-isolated tab and SHALL NOT navigate the serving page.

#### Scenario: A row opens in the desktop's browser

- **WHEN** the user activates a row in the desktop application
- **THEN** the pull request's web page opens in the system browser
- **AND** the SpecForge window does not navigate

#### Scenario: A URL outside the snapshot is refused

- **WHEN** `open_pull_request` is invoked with a URL that is not the web URL of any row in the current BitBucket or GitHub snapshot
- **THEN** the command returns an error and nothing is opened

#### Scenario: The web transport cannot open a pull request on the host

- **WHEN** `open_pull_request` is sent to the web transport's dispatch surface
- **THEN** it is reported as an unknown command and nothing is opened on the serving host

#### Scenario: A row opens a new tab in the browser skin

- **WHEN** the user activates a row in the browser skin
- **THEN** the pull request's web page opens in a new tab with `noopener noreferrer` semantics
- **AND** the SpecForge page itself does not navigate
