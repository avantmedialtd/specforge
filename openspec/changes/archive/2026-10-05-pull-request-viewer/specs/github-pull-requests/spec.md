## MODIFIED Requirements

### Requirement: GitHub Polling With Caching and Backoff

While enabled, the system SHALL refresh on an interval governed by a persisted `github.refreshSecs` setting that defaults to 120 seconds and is floored at 60 seconds, SHALL cache the latest snapshot, SHALL NOT keep more than one refresh in flight, and SHALL run off the UI thread.

$$\text{interval} = \max(\text{refreshSecs},\ 60)$$

A transient or rate-limited reply SHALL keep the previous rows of both lists and mark the snapshot stale; when there are no previous rows the snapshot SHALL be unavailable. An unauthenticated or unavailable outcome SHALL replace the snapshot with that state. A response that cannot be parsed SHALL NOT crash or block other features. The system SHALL issue no request while disabled.

The deferral a rate-limited reply causes SHALL be a **deadline** that belongs to the provider, not to the poller, and that every detail read checks (see the *Shared Backoff and Detail Budget* requirement in the `pull-request-viewer` capability). The system SHALL keep two such deadlines for GitHub, because GraphQL and REST draw on separate primary rate limits. Each is set by the delay of the *GitHub Failure Classification* requirement, so neither is ever more than one hour ahead:

- the **GraphQL deadline**, set by a rate-limited reply to the poller's query or to a detail read's GraphQL query;
- the **REST deadline**, set by a rate-limited reply to a detail read's files request that does not report a secondary rate limit, whether or not it names its resource (`x-ratelimit-resource: core`).

A rate-limited reply that reports a secondary rate limit, to any of these requests, SHALL set both deadlines. The poller SHALL send nothing before the GraphQL deadline, whichever request set it. It SHALL NOT wait on the REST deadline, on the viewer's hourly detail budget or on the viewer's reads in flight, so a REST quota spent by other tools on the same token, or a spent detail budget, never stalls the panel. A detail read's reply SHALL NOT change the snapshot: a rate-limited one only sets deadlines, and its other outcomes belong to the read (see the *GitHub Detail Reads* requirement in the `pull-request-viewer` capability).

$$\text{send}_{\text{poller}}(t) \implies \text{enabled} \;\wedge\; t \ge \text{deadline}_{\text{GraphQL}}$$

Both deadlines SHALL survive disabling and re-enabling the feature and saving a token: neither event SHALL reset them. Re-enabling the feature while the GraphQL deadline holds SHALL publish and announce an unavailable snapshot at once, as a rate-limited refresh with no previous rows does, and the first refresh SHALL then wait out the deadline. The panel and any pull-request address therefore say that GitHub is unavailable rather than loading.

#### Scenario: Periodic refresh

- **WHEN** the feature is enabled and the refresh interval elapses
- **THEN** the system issues one request and replaces the cached snapshot on success

#### Scenario: Offline keeps the last snapshot

- **WHEN** a refresh fails with a transport error and a previous snapshot with rows exists
- **THEN** the panel continues to show the previous rows of both sections, de-emphasised as stale

#### Scenario: A tiny interval is floored

- **WHEN** the refresh interval is set below 60 seconds
- **THEN** refreshes occur no more often than every 60 seconds

#### Scenario: A detail query's rate limit holds the poller

- **WHEN** a detail read's GraphQL query is answered HTTP 429 with `Retry-After: 600`, with the refresh interval at its 120-second default
- **THEN** the poller sends no request until 600 seconds after that reply

#### Scenario: A secondary limit on a files request holds the poller

- **WHEN** a detail read's files request is answered HTTP 403 with no `Retry-After` header, an `x-ratelimit-remaining` of `4000`, and a body reporting that a secondary rate limit was exceeded
- **THEN** the GraphQL deadline and the REST deadline are both set 300 seconds ahead
- **AND** the poller sends no request for 300 seconds

#### Scenario: A spent REST quota does not hold the poller

- **WHEN** no GraphQL deadline holds, and a detail read's files request is answered HTTP 403 with `x-ratelimit-resource: core`, `x-ratelimit-remaining: 0` and an `x-ratelimit-reset` 900 seconds in the future
- **THEN** the REST deadline is set 900 seconds ahead and no GraphQL deadline is set
- **AND** the poller's next refresh is sent when the refresh interval elapses

#### Scenario: The poller's rate limit holds the detail reads

- **WHEN** the poller's query is answered HTTP 429 with `Retry-After: 300`
- **THEN** for 300 seconds no detail read sends a request to GitHub, neither its GraphQL query nor a files request

#### Scenario: A detail read's failure leaves the snapshot unchanged

- **WHEN** the snapshot is fresh, and a detail read's query answers HTTP 401 or one of its files requests answers HTTP 404
- **THEN** the snapshot keeps the rows of both lists and is not marked unauthenticated, unavailable or stale
- **AND** the outcome belongs to that read alone

#### Scenario: A deadline survives switching the feature off and on

- **WHEN** the GraphQL deadline is 600 seconds ahead and the user disables the feature and then enables it again
- **THEN** an unavailable snapshot is published and announced at once, and the panel shows its unavailable line
- **AND** the poller sends no request until the deadline passes

#### Scenario: Saving a token keeps the deadline

- **WHEN** the GraphQL deadline is 600 seconds ahead and the user saves a new token
- **THEN** the poller sends no request until the deadline passes

### Requirement: Opening a GitHub Pull Request

Activating a GitHub row, in either list, SHALL open the pull request in SpecForge by the same rule as a BitBucket row (see the *Opening a Pull Request* requirement in the `bitbucket-pull-requests` capability):

- **A click, Enter or Space** SHALL show the pull request in the main window's center pane at its pull-request address, `/pr/github/<owner>/<repo>/<number>`, adding a history entry (see the *Pull-Request Addresses* requirement in the `view-routing` capability).
- **A click held with the platform's new-window modifier** — Cmd on macOS, Ctrl elsewhere — SHALL open the pull request in its own window: a desktop window in the desktop application, a browser tab in the browser skin. A window or tab already open for that pull request SHALL be brought to the front and focused rather than a second opened, and the gesture SHALL leave the launching view unchanged. The modifier SHALL be selected by platform: on macOS a Ctrl-click is the secondary click, and SHALL open nothing.

A row whose URL is empty SHALL open nothing (see *GitHub Row Signals*).

The pull request's web page SHALL open from the "Open on GitHub" control in the pull-request view's header (see the *Pull-Request View* requirement in the `pull-request-viewer` capability), by the same means as a BitBucket pull request's: on the desktop, for a pull request listed in the current GitHub snapshot, through the `open_pull_request` command, which SHALL accept the web URL of any row in either list of the current GitHub snapshot; in the browser skin as a link opening a new opener-isolated tab. The web transport SHALL still expose no `open_pull_request` command.

#### Scenario: A review-requested row opens on the desktop

- **WHEN** the user clicks a row in the "To review" section in the desktop application
- **THEN** the center pane shows that pull request at its address `/pr/github/<owner>/<repo>/<number>`
- **AND** the view's header offers "Open on GitHub", which opens the pull request's web page in the system browser without navigating the window

#### Scenario: A row opens a new tab in the browser skin

- **WHEN** the user clicks a GitHub row in the browser skin holding Cmd on macOS or Ctrl elsewhere
- **THEN** the pull request opens in a tab of its own, at its pull-request address
- **AND** the SpecForge page itself does not navigate
- **AND** repeating the gesture brings that tab to the front rather than opening another

#### Scenario: A modifier click opens a pull-request window on the desktop

- **WHEN** an artifact is displayed and the user clicks a row in the "Yours" section of the desktop application holding Cmd on macOS or Ctrl elsewhere
- **THEN** the pull request opens in its own window
- **AND** the center pane still shows the artifact, and the navigation history is unchanged

#### Scenario: The secondary click opens nothing on macOS

- **WHEN** the user Ctrl-clicks a GitHub row on macOS, which is that platform's secondary click
- **THEN** no window or tab opens for the pull request
- **AND** the center pane does not change

### Requirement: GitHub Privacy and Safety

The system SHALL send the GitHub token only to `https://api.github.com`, and only in the `Authorization` header of the poller's request named in *One Constant Read-Only Query* and of the pull-request viewer's detail reads (see the *GitHub Detail Reads* requirement in the `pull-request-viewer` capability). A detail read SHALL consist only of:

- a `POST` to `https://api.github.com/graphql` carrying a read-only query fixed at build time, never a mutation or subscription; and
- `GET` requests for the pages of `https://api.github.com/repos/{owner}/{name}/pulls/{number}/files`.

A detail read SHALL be sent only for a pull request listed in the current GitHub snapshot. Its owner, repository name and number SHALL be taken from that row, never from a URL or any other value the caller supplies, and SHALL reach the query only through its GraphQL `variables`, so the query's text stays byte-identical whichever pull request it reads.

The token SHALL NOT be written to logs or diagnostic output and SHALL NOT be sent through an ambient proxy configuration. No request SHALL follow a redirect. All GitHub network activity SHALL occur only while the feature is enabled: a detail read SHALL re-check the enabled flag before each of its requests, and SHALL send nothing more once the feature is disabled. No other GitHub host SHALL be contacted.

#### Scenario: Only the official endpoint

- **WHEN** the poller issues any request
- **THEN** its URL is `https://api.github.com/graphql` and no other destination receives the token

#### Scenario: Token never logged

- **WHEN** the poller or a detail read builds and sends a request, or a request fails
- **THEN** the token value appears in no log line or diagnostic output

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made, by the poller or by a detail read, regardless of the stored token, the environment, or the refresh interval

#### Scenario: Detail reads stay on the API host

- **WHEN** a detail read runs for pull request 42 of `acme/api`, which the GitHub snapshot lists
- **THEN** each of its requests is either the detail query sent as a `POST` to `https://api.github.com/graphql` or a `GET` of a page of `https://api.github.com/repos/acme/api/pulls/42/files`
- **AND** no other destination receives the token

#### Scenario: A pull request outside the snapshot is not read

- **WHEN** `get_pull_request_detail` is invoked for a GitHub pull request that matches no row of the current GitHub snapshot
- **THEN** no request is sent to GitHub

#### Scenario: A detail read follows no redirect

- **WHEN** GitHub answers a detail read's files request with a redirect
- **THEN** the redirect is not followed and the token is sent nowhere else
- **AND** the read reports that pull request as unavailable

#### Scenario: Disabling stops a detail read between requests

- **WHEN** the feature is disabled while a detail read is between two of its requests
- **THEN** the read sends no further request
- **AND** its result is neither cached nor returned
