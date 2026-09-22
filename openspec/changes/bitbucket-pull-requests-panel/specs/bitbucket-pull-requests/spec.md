## ADDED Requirements

### Requirement: Opt-in Pull-Request Tracking

The system SHALL provide a "BitBucket pull requests" feature that is disabled by default and controlled by a persisted `bitbucket.enabled` setting. Credential resolution, polling and the pull-request panel SHALL be active only while the setting is enabled. Disabling the setting SHALL stop polling and remove the panel from every frontend that renders it, without a restart.

#### Scenario: Disabled by default on first run

- **WHEN** a user opens SpecForge with no prior `bitbucket` settings block
- **THEN** the feature is off, no panel is rendered, no credential is read, and no request is made to BitBucket

#### Scenario: Enabling starts tracking

- **WHEN** the user enables the setting
- **THEN** the system begins polling and renders the panel once a snapshot is available

#### Scenario: Disabling stops tracking

- **WHEN** the user disables the setting
- **THEN** the system stops polling, collapses the snapshot to the disabled state, and the panel disappears from the desktop and the browser skin

### Requirement: Credentials Are Stored Write-Only

The system SHALL accept a BitBucket username (or email) and a BitBucket-specific API token from Settings and persist both in the application settings beside the other preferences. No command on any transport SHALL return the token: the configuration getter SHALL report only whether a token is set, together with the username, the enabled flag, the refresh interval and the panel position. Setting an empty token SHALL clear the stored token.

When both `BITBUCKET_USERNAME` and `BITBUCKET_API_TOKEN` are present in the process environment they SHALL take precedence over the stored pair, so a headless server can be configured without the settings UI.

The Settings copy SHALL state that a Jira or Confluence API token is not accepted by BitBucket Cloud and SHALL point at where a BitBucket token is created, naming the read scopes the recipe needs: account, workspace membership and pull requests.

#### Scenario: The token is never read back

- **WHEN** a frontend requests the BitBucket configuration
- **THEN** the response carries the username and a boolean stating whether a token is set
- **AND** the token value is absent from the response

#### Scenario: Replacing the credentials

- **WHEN** the user submits a username and token in Settings
- **THEN** both are persisted and the next refresh uses them, without restarting the application

#### Scenario: Clearing the token

- **WHEN** the user submits an empty token
- **THEN** the stored token is removed and the next refresh reports the unauthenticated state

#### Scenario: Environment variables override the stored pair

- **WHEN** `BITBUCKET_USERNAME` and `BITBUCKET_API_TOKEN` are both set in the process environment
- **THEN** the poller authenticates with the environment pair regardless of what is stored

#### Scenario: A partial environment does not override

- **WHEN** only one of the two environment variables is set
- **THEN** the stored pair is used

### Requirement: Authored Pull-Request Discovery

While enabled and authenticated, the system SHALL list the pull requests authored by the configured account across every workspace it belongs to using only HTTP GET requests: `GET /2.0/user` to resolve the account, `GET /2.0/user/workspaces` to enumerate workspaces, and `GET /2.0/workspaces/{workspace}/pullrequests/{account}` per workspace. The account identifier SHALL be the account's `uuid` when present, else its `account_id`. The system SHALL NOT call the removed `GET /2.0/pullrequests/{selected_user}` endpoint.

Each per-workspace request SHALL carry `state=OPEN`, `sort=-updated_on`, a page length of 50, and the `fields` partial-response parameter adding `participants` and `reviewers` to the list values, with every `+` in that parameter percent-encoded as `%2B`. Only the first page of each workspace's pull requests SHALL be fetched. The workspace listing SHALL request a page length of 100 and SHALL follow the page's `next` link while one is present, up to a fixed bound of five pages, following only links that point back at the official API. For an account in at most 100 workspaces the number of requests per refresh is therefore

$$\text{requests} = 2 + W$$

for an account in $W$ workspaces.

A workspace whose request answers HTTP 403 or 404 SHALL be skipped and named by slug in the snapshot; the remaining workspaces SHALL still be listed. Results from all workspaces SHALL be merged and ordered by `updated_on`, most recent first.

#### Scenario: Pull requests are listed from every workspace

- **WHEN** the account belongs to workspaces `W1` and `W2`
- **THEN** the account is resolved via `GET /2.0/user`, the workspaces via `GET /2.0/user/workspaces`, and `GET /2.0/workspaces/{ws}/pullrequests/{account}` is requested for both with `state=OPEN`
- **AND** the snapshot contains the open pull requests authored by the account in both workspaces

#### Scenario: The review fields are requested encoded

- **WHEN** a per-workspace request URL is built
- **THEN** its `fields` parameter contains `%2Bvalues.participants` and `%2Bvalues.reviewers`
- **AND** contains no literal `+`

#### Scenario: An inaccessible workspace is skipped

- **WHEN** the request for `W2` answers HTTP 403 or 404
- **THEN** `W2` is listed among the snapshot's skipped workspaces
- **AND** the pull requests from `W1` are still present in the snapshot

#### Scenario: Results are merged newest first

- **WHEN** pull requests are returned from more than one workspace
- **THEN** the snapshot's rows are ordered by their updated time, most recent first, regardless of workspace

#### Scenario: A workspace listing spanning pages is followed

- **WHEN** `GET /2.0/user/workspaces` answers with a `next` link
- **THEN** the linked page is requested with the same credential and its workspaces are listed too
- **AND** a `next` link that does not point at the official API is not followed

#### Scenario: Only the first page is fetched

- **WHEN** a workspace has more than 50 open authored pull requests
- **THEN** its 50 most recently updated are listed
- **AND** no further page is requested

### Requirement: Review-State Summary

For each pull request the snapshot SHALL carry the repository's full name, the id, title, source and destination branch names, the web URL, the draft flag, the updated time as Unix epoch seconds, the open-task count, and a review summary of: approvals, changes requested, and pending reviewers. The author's own approval or change request SHALL NOT be counted. Pending SHALL be the number of entries in `reviewers` that have neither approved nor requested changes in `participants`. Accounts SHALL be matched by `account_id`, falling back to `uuid`. When the response carries no `participants` the review summary SHALL be absent, so an unknown review state is distinguishable from one with no activity.

#### Scenario: The author's approval is excluded

- **WHEN** a pull request's participants include the author with `approved: true` and one other participant with `approved: true`
- **THEN** the summary reports one approval

#### Scenario: Pending reviewers are counted from the reviewers list

- **WHEN** a pull request lists three reviewers, one of whom has approved and one of whom requested changes
- **THEN** the summary reports one approval, one change request, and one pending reviewer

#### Scenario: A response without participants yields no summary

- **WHEN** a pull request in the list response carries no `participants` field
- **THEN** the row's review summary is absent rather than all zeros

### Requirement: Polling With Caching and Backoff

While enabled, the system SHALL refresh on an interval governed by a persisted `bitbucket.refreshSecs` setting that defaults to 120 seconds and is floored at 60 seconds. It SHALL cache the latest snapshot, SHALL NOT keep more than one refresh in flight, SHALL honour an HTTP 429 `Retry-After` by deferring the next refresh until the hinted delay elapses (defaulting to 300 seconds when absent), and SHALL run off the UI thread. An HTTP 401 from any request, or an HTTP 403 from the account resources (`GET /2.0/user`, `GET /2.0/user/workspaces`), SHALL yield the unauthenticated state, because a 403 there means the token exists but lacks a read scope the recipe needs, which is corrected in Settings rather than by waiting. A transport error, a 429, or any other non-success status not covered above SHALL keep the previous rows and mark the snapshot stale; when there are no previous rows the snapshot SHALL be unavailable. A response that cannot be parsed SHALL yield the unavailable state and SHALL NOT crash or block other features. The system SHALL issue no request while disabled.

#### Scenario: Periodic refresh

- **WHEN** the feature is enabled and the refresh interval elapses
- **THEN** the system issues one request chain and replaces the cached snapshot on success

#### Scenario: Rate-limit backoff

- **WHEN** a request answers HTTP 429 with a `Retry-After` hint
- **THEN** the previous rows are kept and marked stale
- **AND** the next refresh is deferred until the hinted delay elapses

#### Scenario: Offline keeps the last snapshot

- **WHEN** a refresh fails with a transport error and a previous snapshot exists
- **THEN** the panel continues to show the previous rows, de-emphasised as stale

#### Scenario: A rejected credential is reported

- **WHEN** any request answers HTTP 401
- **THEN** the snapshot is unauthenticated and the panel prompts the user to check the credentials in Settings

#### Scenario: A token without the account scopes is reported as a credential problem

- **WHEN** `GET /2.0/user` or `GET /2.0/user/workspaces` answers HTTP 403
- **THEN** the snapshot is unauthenticated and the panel prompts the user to check the credentials in Settings
- **AND** the snapshot is not marked unavailable or stale

#### Scenario: Unexpected response shape

- **WHEN** a response body cannot be parsed into the expected shape
- **THEN** the snapshot is unavailable and the rest of the application is unaffected

#### Scenario: A tiny interval is floored

- **WHEN** the refresh interval is set below 60 seconds
- **THEN** refreshes occur no more often than every 60 seconds

### Requirement: The Snapshot Is Announced on the Cache Stream

A changed snapshot SHALL be announced by a `pull-requests-updated` event carrying no payload, derived from a cache-event variant so it reaches the desktop forwarder and the browser skin's event stream through the one shared envelope mapping. Frontends SHALL re-read the snapshot through `get_my_pull_requests` on receipt. An unchanged snapshot SHALL NOT be announced.

#### Scenario: A refresh that changes nothing is silent

- **WHEN** a refresh yields a snapshot equal to the cached one
- **THEN** no event is emitted

#### Scenario: The browser skin receives the announcement

- **WHEN** the snapshot changes while the browser skin is connected
- **THEN** the browser skin receives `pull-requests-updated` over its event stream and re-reads the snapshot

### Requirement: Pull-Request Panel

When the feature is enabled, the desktop application and the browser skin SHALL render a pull-request panel: a collapsible section whose header shows a title, the number of open pull requests, and a disclosure control, and whose body lists one row per pull request in snapshot order. Each row SHALL show the repository's full name, the title, the source and destination branch, a review cell (approvals, changes requested, pending, in that order, with an "unknown" treatment when the summary is absent), the open-task count when non-zero, a draft marker when the pull request is a draft, and the updated time relative to now. A stale snapshot SHALL be rendered de-emphasised. When the snapshot is unauthenticated, unavailable, or empty, the body SHALL show one quiet line stating which, and the unauthenticated line SHALL point to Settings. When the feature is disabled the panel SHALL NOT be rendered at all. Skipped workspaces SHALL be visible on request — for example in the header's tooltip — and SHALL NOT be rendered as an error.

The collapsed-or-expanded state of the panel SHALL be per-viewer frontend view state, persisted like pane visibility and never stored in application settings; a collapsed panel SHALL keep its one-line header so the count stays visible. The panel's body SHALL be bounded in height and scroll internally (see the *Side Panes Host the Pull-Request Panel* requirement in the `spec-browser` capability).

#### Scenario: Rows render the summary

- **WHEN** the snapshot is ok and holds a pull request with two approvals, no change requests, one pending reviewer and three open tasks
- **THEN** its row shows the repository, title, branches, a review cell reading two approvals, zero change requests and one pending, the count three for tasks, and a relative updated time

#### Scenario: A stale snapshot is de-emphasised

- **WHEN** the snapshot is marked stale
- **THEN** the rows remain visible and the panel is rendered in its de-emphasised treatment

#### Scenario: The unauthenticated state points to Settings

- **WHEN** the snapshot is unauthenticated
- **THEN** the panel body shows a single line saying the credentials need attention and referring to Settings

#### Scenario: No open pull requests

- **WHEN** the snapshot is ok and holds no rows
- **THEN** the panel shows its header with a count of zero and a single quiet line stating there are no open pull requests

#### Scenario: Collapsing persists for this viewer only

- **WHEN** the user collapses the panel and reloads the application
- **THEN** the panel is still collapsed, its header and count still visible
- **AND** the application settings are unchanged

### Requirement: Panel Position Is a Persisted Setting

The panel's position SHALL be a persisted `bitbucket.panelPosition` setting drawn from exactly four values — `left-top`, `left-bottom`, `right-top`, `right-bottom` — with the default `left-bottom`. It SHALL be chosen in Settings, in both the desktop application and the browser skin, SHALL survive a restart, and SHALL NOT be stored per window. A change SHALL be announced by a dedicated `pull-request-panel-moved` event carrying the new position on both the desktop and the browser event transports, so windows already open re-seat the panel without reopening. A stored value the running version does not recognise SHALL load as the default and SHALL NOT fail the load of the settings as a whole.

#### Scenario: A chosen position survives a restart

- **WHEN** the user selects `right-top` and restarts the application
- **THEN** the panel renders above the commit rail from the first frame

#### Scenario: An open window follows the change

- **WHEN** the position is changed in Settings while another window or a connected browser skin is open
- **THEN** that window re-seats the panel at the new position without being reopened

#### Scenario: An unknown position degrades to the default

- **WHEN** the settings file carries a panel position the running version does not recognise
- **THEN** the panel renders at `left-bottom`
- **AND** every other setting in the file loads intact

### Requirement: Opening a Pull Request

Activating a row SHALL open the pull request's BitBucket web page. In the desktop application this SHALL go through a dedicated `open_pull_request` command whose service layer accepts only a URL that is the web URL of a row in the current snapshot and refuses any other value, and which hands the accepted URL to the platform opener; the frontend SHALL NOT thereby gain a general open-URL capability. The command SHALL be absent from the web transport's dispatch surface, consistent with the *Link Handling in the Browser Skin* requirement in the `web-ui` capability. In the browser skin a row SHALL be a link that opens in a new opener-isolated tab and SHALL NOT navigate the serving page.

#### Scenario: A row opens in the desktop's browser

- **WHEN** the user activates a row in the desktop application
- **THEN** the pull request's web page opens in the system browser
- **AND** the SpecForge window does not navigate

#### Scenario: A URL outside the snapshot is refused

- **WHEN** `open_pull_request` is invoked with a URL that is not the web URL of any row in the current snapshot
- **THEN** the command returns an error and nothing is opened

#### Scenario: The web transport cannot open a pull request on the host

- **WHEN** `open_pull_request` is sent to the web transport's dispatch surface
- **THEN** it is reported as an unknown command and nothing is opened on the serving host

#### Scenario: A row opens a new tab in the browser skin

- **WHEN** the user activates a row in the browser skin
- **THEN** the pull request's web page opens in a new tab with `noopener noreferrer` semantics
- **AND** the SpecForge page itself does not navigate

### Requirement: Privacy and Safety

The system SHALL send the BitBucket credential only to `https://api.bitbucket.org` and only in the `Authorization` header of the requests named in *Authored Pull-Request Discovery*. The token SHALL NOT be written to logs or diagnostic output, SHALL NOT be sent through an ambient proxy configuration, and all BitBucket network activity SHALL occur only while the feature is enabled.

#### Scenario: Token never logged

- **WHEN** the poller builds and sends a request
- **THEN** the token value appears in no log line or diagnostic output

#### Scenario: Only the official endpoint

- **WHEN** the poller issues any request
- **THEN** its host is `api.bitbucket.org` and no other destination receives the credential

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made regardless of the stored credentials or the refresh interval

### Requirement: The Terminal Frontend Does Not Render the Panel

The terminal frontend SHALL expose the enabled toggle on its Settings screen (see the *Terminal Settings Screen* requirement in the `terminal-ui` capability) but SHALL NOT start the poller and SHALL NOT render a pull-request list. The absence is deliberate: a terminal list is a separate change. The terminal frontend SHALL load a settings file carrying a `bitbucket` block without error.

#### Scenario: The terminal loads the settings block

- **WHEN** the terminal frontend starts with a settings file containing a `bitbucket` block
- **THEN** it starts successfully and makes no request to BitBucket

#### Scenario: The terminal flips the shared toggle

- **WHEN** the user flips the BitBucket pull-requests toggle on the terminal's Settings screen
- **THEN** `bitbucket.enabled` is written to the shared application settings
- **AND** the terminal itself still renders no pull-request list
