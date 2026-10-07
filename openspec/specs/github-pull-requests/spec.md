# github-pull-requests Specification

## Purpose

Defines the opt-in GitHub pull-request panel for SpecForge, the structural twin of the BitBucket one (see the `bitbucket-pull-requests` capability): a background poller in the headless app layer that sends one read-only GraphQL query, fixed at build time, to `https://api.github.com/graphql` on each refresh and lists the viewer's open authored pull requests and those awaiting their review; a single write-only token stored in the application settings and overridable from `GH_TOKEN` then `GITHUB_TOKEN`; the row signals the query carries (review summary, checks, conflicts, unresolved conversations, author); the GitHub-shaped failure classification (rate limits signalled by headers, bodies and 200 errors, a missing scope, withheld results) with its capped backoff; the snapshot's own cache event; the two-section panel rendered in one of four persisted side-pane slots, independent of the BitBucket panel's; how a row is opened on each transport; the privacy posture; and the terminal frontend's deliberate toggle-only stance.
## Requirements
### Requirement: Opt-in GitHub Pull-Request Tracking

The system SHALL provide a "GitHub pull requests" feature that is disabled by default and controlled by a persisted `github.enabled` setting, independent of the BitBucket pull-request feature (see the *Opt-in Pull-Request Tracking* requirement in the `bitbucket-pull-requests` capability): either, both or neither MAY be enabled. Token resolution, polling and the GitHub pull-request panel SHALL be active only while the setting is enabled. Disabling the setting SHALL stop polling and remove the GitHub panel from every frontend that renders it, without a restart, and SHALL leave the BitBucket panel as it was.

#### Scenario: Disabled by default on first run

- **WHEN** a user opens SpecForge with no prior `github` settings block
- **THEN** the feature is off, no GitHub panel is rendered, no token is read, and no request is made to GitHub

#### Scenario: Enabling starts tracking

- **WHEN** the user enables the setting
- **THEN** the system begins polling and renders the GitHub panel once a snapshot is available

#### Scenario: Disabling stops tracking

- **WHEN** the user disables the setting
- **THEN** the system stops polling, collapses the GitHub snapshot to the disabled state, and the GitHub panel disappears from the desktop and the browser skin

#### Scenario: The two providers are independent

- **WHEN** the BitBucket feature is enabled and the GitHub feature is disabled
- **THEN** the BitBucket panel renders and no request is made to GitHub
- **AND** enabling GitHub afterwards adds the GitHub panel without changing the BitBucket one

### Requirement: The GitHub Token Is Stored Write-Only

The system SHALL accept a single GitHub token from Settings and persist it in the application settings beside the other preferences. No command on any transport SHALL return the token: the configuration getter SHALL report only whether a token is set, together with the enabled flag, the refresh interval and the panel position. Setting an empty token SHALL clear the stored token.

The poller SHALL resolve the token in this order, skipping a variable that is absent or empty: the `GH_TOKEN` environment variable, then the `GITHUB_TOKEN` environment variable, then the stored token. When none yields a token the snapshot SHALL be unauthenticated.

The Settings copy SHALL state that a classic token needs the `repo` scope to see private repositories and that this scope also permits writes, which SpecForge never performs; that a fine-grained token can be read-only but sees the repositories of a single account or organisation; that a token must be authorised for each organisation enforcing single sign-on; that `gh auth token` prints a token the GitHub CLI already holds; and SHALL point at where a GitHub token is created.

#### Scenario: The token is never read back

- **WHEN** a frontend requests the GitHub configuration
- **THEN** the response carries a boolean stating whether a token is set
- **AND** the token value is absent from the response

#### Scenario: Replacing the token

- **WHEN** the user submits a token in Settings
- **THEN** it is persisted and the next refresh uses it, without restarting the application

#### Scenario: Clearing the token

- **WHEN** the user submits an empty token and neither environment variable is set
- **THEN** the stored token is removed and the next refresh reports the unauthenticated state

#### Scenario: GH_TOKEN takes precedence

- **WHEN** `GH_TOKEN` and `GITHUB_TOKEN` are both set to non-empty values and a token is stored
- **THEN** the poller authenticates with `GH_TOKEN`

#### Scenario: GITHUB_TOKEN is the fallback

- **WHEN** `GH_TOKEN` is absent or empty and `GITHUB_TOKEN` is set
- **THEN** the poller authenticates with `GITHUB_TOKEN` regardless of what is stored

### Requirement: One Constant Read-Only Query

While enabled and holding a token, each refresh SHALL issue exactly one HTTP request: a `POST` to `https://api.github.com/graphql` whose body carries a single GraphQL query fixed at build time. The query SHALL be a read-only operation — never a mutation or subscription — and no value supplied at runtime SHALL be interpolated into it.

The query SHALL list two sets of open pull requests:

- **authored** — the open pull requests authored by the token's account, across every repository the token can read, most recently updated first; and
- **review requested** — the open pull requests on which a review is requested from the token's account, directly or through a team it belongs to, most recently updated first.

Each list SHALL be limited to its first 50 entries and no further page SHALL be requested. Pull requests in archived repositories SHALL be excluded from both lists. A pull request present in both sets — possible when a team the account belongs to is requested on the account's own pull request — SHALL be listed once, in the authored set, so no web URL appears twice in the snapshot. Each list SHALL be ordered by its updated time, most recent first, when stored in the snapshot.

$$\text{requests per refresh} = 1$$

#### Scenario: One request lists both sets

- **WHEN** a refresh runs with a valid token
- **THEN** exactly one request is sent, to `https://api.github.com/graphql`
- **AND** the snapshot carries the account's open authored pull requests and the open pull requests awaiting its review

#### Scenario: The query cannot write

- **WHEN** the query sent by the poller is inspected
- **THEN** it is a `query` operation containing no `mutation` or `subscription`
- **AND** it is byte-identical on every refresh

#### Scenario: A team review request is listed

- **WHEN** a review is requested from a team the account belongs to, and not from the account directly
- **THEN** that pull request appears in the review-requested set

#### Scenario: Archived repositories are excluded

- **WHEN** the account authored an open pull request in a repository that has since been archived
- **THEN** that pull request appears in neither set

#### Scenario: A pull request in both sets is listed once

- **WHEN** the account's own open pull request has a review requested from a team the account belongs to
- **THEN** it appears in the authored set only

#### Scenario: Only the first 50 are listed

- **WHEN** the account has more than 50 open authored pull requests
- **THEN** the 50 most recently updated are listed and no further request is made

### Requirement: GitHub Row Signals

Each row SHALL carry the repository's `owner/name`, the pull-request number, title, head and base branch names, the web URL, the draft flag, the updated time as Unix epoch seconds, the author's login when GitHub reports one, and:

- a **review summary** of approvals, changes requested and pending reviews, where approvals and changes requested count the latest opinionated review of each reviewer other than the author in the `APPROVED` and `CHANGES_REQUESTED` states respectively, and pending counts the outstanding review requests, whether to a user or a team; the summary SHALL be absent when the response lacked either input, so an unknown review state stays distinct from one with no activity;
- a **checks state** derived from the check rollup of the pull request's latest commit: `SUCCESS` is passing; `FAILURE` and `ERROR` are failing; `PENDING` and `EXPECTED` are pending; no rollup means no checks state;
- a **conflicting** flag that is true only when GitHub reports the pull request's mergeability as `CONFLICTING`, so an as-yet-uncomputed mergeability reads as not conflicting rather than as unknown; and
- the number of **unresolved review conversations**, counted among the first 100 review threads.

A row whose web URL does not begin with `https://github.com/` SHALL carry an empty URL and SHALL NOT be openable.

#### Scenario: A reviewed pull request summarises its reviews

- **WHEN** a pull request's latest opinionated reviews are one approval and one change request, and two review requests are outstanding
- **THEN** its summary reports one approval, one change request and two pending

#### Scenario: Checks map onto three states

- **WHEN** the latest commit's check rollup is `ERROR`
- **THEN** the row's checks state is failing
- **AND** a rollup of `EXPECTED` is pending, and a missing rollup is no checks state

#### Scenario: Uncomputed mergeability is not a conflict

- **WHEN** GitHub reports the mergeability as `UNKNOWN`
- **THEN** the row is not conflicting

#### Scenario: Unresolved conversations are counted

- **WHEN** a pull request has three review threads, one of which is resolved
- **THEN** the row reports two unresolved conversations

#### Scenario: A foreign link is not openable

- **WHEN** a row's URL does not begin with `https://github.com/`
- **THEN** the row's URL is empty and activating it opens nothing

### Requirement: GitHub Failure Classification

The poller SHALL classify each reply before reading rows:

- a transport error, a redirect, or any non-success status not listed below SHALL be transient;
- HTTP 403 or 429 carrying a rate-limit signal — a `Retry-After` header, an `x-ratelimit-remaining` header of `0`, or a body reporting a primary or secondary rate limit — SHALL be rate-limited, as SHALL an HTTP 429 without any of these, and as SHALL a successful reply whose body reports an error of type `RATE_LIMITED`;
- HTTP 401, and HTTP 403 without any rate-limit signal, SHALL be unauthenticated;
- a successful reply whose body carries no usable `data` SHALL be unauthenticated when its errors include one of type `INSUFFICIENT_SCOPES` — the token exists but lacks a scope the query needs, which is corrected in Settings rather than by waiting — and unavailable otherwise.

A rate-limited reply, whatever its status, SHALL defer the next refresh by the reply's `Retry-After` seconds when present, else — only when its `x-ratelimit-remaining` is `0` — until its `x-ratelimit-reset` time, else by 300 seconds; and never by more than one hour. GitHub sends a reset on every reply, but it dates the primary window, so a secondary limit (whose primary quota is not spent) SHALL NOT wait for it. The cap keeps a hostile or corrupt header from stalling or crashing the poller.

$$\text{delay} = \min\left(3600,\ \begin{cases} \text{Retry-After} & \text{if present} \\ \max(\text{reset} - \text{now},\ 0) & \text{else if remaining} = 0 \text{ and reset present} \\ 300 & \text{otherwise} \end{cases}\right)$$

A successful reply that carries usable `data` together with errors SHALL still be read: an entry GitHub withheld (returned as null) SHALL be omitted from its list and counted in the snapshot's withheld count, and the remaining entries SHALL be listed.

#### Scenario: An exhausted quota on a 403 backs off

- **WHEN** a reply is HTTP 403 with `x-ratelimit-remaining: 0` and an `x-ratelimit-reset` 600 seconds in the future, with the refresh interval at its 120-second default
- **THEN** the snapshot keeps its previous rows marked stale
- **AND** no further request is made for 600 seconds
- **AND** the snapshot is not unauthenticated

#### Scenario: A rate limit reported in the body backs off by the headers

- **WHEN** a reply is HTTP 200 whose body carries an error of type `RATE_LIMITED`, with `x-ratelimit-remaining: 0` and an `x-ratelimit-reset` 900 seconds in the future
- **THEN** the reply is treated as rate-limited, not as unavailable
- **AND** no further request is made for 900 seconds

#### Scenario: A secondary rate limit is not a credential problem

- **WHEN** a reply is HTTP 403 with no `Retry-After` header, an `x-ratelimit-remaining` of `4000`, an `x-ratelimit-reset` 55 minutes in the future, and a body reporting that a secondary rate limit was exceeded
- **THEN** the reply is rate-limited and the next refresh is deferred by 300 seconds, not until the reset
- **AND** the snapshot is not unauthenticated

#### Scenario: A hostile delay is capped

- **WHEN** a rate-limited reply carries `Retry-After: 18446744073709551615`
- **THEN** the next refresh is deferred by one hour
- **AND** the poller keeps running

#### Scenario: A plain 403 is a credential problem

- **WHEN** a reply is HTTP 403 with no `Retry-After` header, a non-zero or absent `x-ratelimit-remaining`, and a body reporting no rate limit
- **THEN** the snapshot is unauthenticated and the panel points to Settings

#### Scenario: A missing scope is a credential problem

- **WHEN** a reply is HTTP 200 with no `data` and an error of type `INSUFFICIENT_SCOPES`
- **THEN** the snapshot is unauthenticated and the panel points to Settings

#### Scenario: Withheld entries are counted, not fatal

- **WHEN** a reply carries data in which two review-requested entries are null, together with errors explaining why
- **THEN** the other entries are listed
- **AND** the snapshot's withheld count is two

#### Scenario: A redirect is not followed

- **WHEN** GitHub answers the request with a redirect
- **THEN** the redirect is not followed, the credential is sent nowhere else, and the reply is transient

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

### Requirement: The GitHub Snapshot Is Announced on the Cache Stream

A changed GitHub snapshot SHALL be announced by a `github-pull-requests-updated` event carrying no payload, derived from its own cache-event variant so it reaches the desktop forwarder and the browser skin's event stream through the one shared envelope mapping. Frontends SHALL re-read the snapshot through `get_github_pull_requests` on receipt. An unchanged snapshot SHALL NOT be announced; a new fetch time alone is not a change. A GitHub refresh SHALL NOT cause the BitBucket panel to re-read its snapshot.

#### Scenario: A refresh that changes nothing is silent

- **WHEN** a refresh yields a snapshot equal to the cached one apart from its fetch time
- **THEN** no event is emitted

#### Scenario: The browser skin receives the announcement

- **WHEN** the GitHub snapshot changes while the browser skin is connected
- **THEN** the browser skin receives `github-pull-requests-updated` over its event stream and re-reads the GitHub snapshot
- **AND** the BitBucket panel does not re-read its own

### Requirement: GitHub Pull-Request Panel

When the feature is enabled, the desktop application and the browser skin SHALL render a GitHub pull-request panel: a collapsible section whose header names GitHub, shows the number of authored rows and the number of review-requested rows, and carries a disclosure control. Its body SHALL list the authored rows under a "Yours" heading and the review-requested rows under a "To review" heading, each in snapshot order; a section with no rows SHALL NOT be rendered.

Each row SHALL show the repository, the title, the head and base branch, the review cell (approvals, changes requested, pending, in that order, with an "unknown" treatment when the summary is absent), a checks indicator when the row has a checks state — distinguishing passing, failing and pending, with the state in words in its tooltip and accessible label — a conflict marker when the row is conflicting, the number of unresolved conversations when non-zero, a draft marker when the pull request is a draft, and the updated time relative to now. Rows in the "To review" section SHALL also show the author's login.

A stale snapshot SHALL be rendered de-emphasised. When the snapshot is unauthenticated or unavailable, the body SHALL show one quiet line stating which, and the unauthenticated line SHALL point to Settings. When both lists are empty the body SHALL show one quiet line stating there is nothing open and nothing to review. When the snapshot's withheld count is non-zero, the header's tooltip SHALL say how many results GitHub withheld, that an organisation may require the token to be authorised for single sign-on, and that a fine-grained token sees only the one account or organisation it was created for; it SHALL NOT be rendered as an error. The panel's accessible name SHALL name GitHub, so it is distinguishable from the BitBucket panel's. When the feature is disabled the panel SHALL NOT be rendered at all.

The collapsed-or-expanded state SHALL be per-viewer frontend view state kept separately from the BitBucket panel's, persisted like pane visibility and never stored in application settings; a collapsed panel SHALL keep its one-line header so both counts stay visible. The body SHALL be bounded in height and scroll internally (see the *Side Panes Host the Pull-Request Panel* requirement in the `spec-browser` capability).

#### Scenario: Two sections with their own rows

- **WHEN** the snapshot holds two authored rows and one review-requested row
- **THEN** the header shows the counts two and one
- **AND** the body shows a "Yours" section with two rows and a "To review" section with one row showing its author

#### Scenario: An empty section is omitted

- **WHEN** the snapshot holds authored rows and no review-requested rows
- **THEN** only the "Yours" section is rendered

#### Scenario: Rows render the GitHub signals

- **WHEN** a row has failing checks, is conflicting, and has two unresolved conversations
- **THEN** it shows a failing checks indicator whose tooltip says the checks are failing, a conflict marker, and the count two for conversations

#### Scenario: No checks renders no indicator

- **WHEN** a row has no checks state
- **THEN** no checks indicator is rendered for it

#### Scenario: Nothing open and nothing to review

- **WHEN** the snapshot is ok and both lists are empty
- **THEN** the header shows zero for both counts and the body shows a single quiet line

#### Scenario: Withheld results are explained on request

- **WHEN** the snapshot's withheld count is three
- **THEN** the header's tooltip states that three results were withheld and mentions single-sign-on authorisation
- **AND** no error treatment is rendered

#### Scenario: Collapsing is independent of the BitBucket panel

- **WHEN** both panels are rendered and the user collapses the GitHub panel and reloads
- **THEN** the GitHub panel is still collapsed with both counts visible
- **AND** the BitBucket panel's collapsed state is unchanged

### Requirement: GitHub Panel Position Is a Persisted Setting

The GitHub panel's position SHALL be a persisted `github.panelPosition` setting drawn from the same four values as the BitBucket panel's — `left-top`, `left-bottom`, `right-top`, `right-bottom` — with the default `left-bottom`, independent of the BitBucket panel's position. It SHALL be chosen in Settings in both the desktop application and the browser skin, SHALL survive a restart, and SHALL NOT be stored per window. A change SHALL be announced by the `pull-request-panel-moved` event carrying `{ provider: "github", position }` on both event transports, so windows already open re-seat the GitHub panel without reopening and leave the BitBucket panel where it is. A stored value the running version does not recognise SHALL load as the default and SHALL NOT fail the load of the settings as a whole.

#### Scenario: A chosen position survives a restart

- **WHEN** the user selects `right-bottom` for GitHub and restarts the application
- **THEN** the GitHub panel renders below the commit rail from the first frame

#### Scenario: A move re-seats only the GitHub panel

- **WHEN** the GitHub position is changed while another window or a connected browser skin is open
- **THEN** that window re-seats the GitHub panel without being reopened
- **AND** the BitBucket panel stays in its own slot

#### Scenario: An unknown position degrades to the default

- **WHEN** the settings file carries a GitHub panel position the running version does not recognise
- **THEN** the GitHub panel renders at `left-bottom`
- **AND** every other setting in the file loads intact

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

The system SHALL send the GitHub token only to `https://api.github.com`, and only in the `Authorization` header of the poller's request named in *One Constant Read-Only Query* and of the pull-request viewer's detail reads and file reads (see the *GitHub Detail Reads* requirement in the `pull-request-viewer` capability). A detail read SHALL consist only of:

- a `POST` to `https://api.github.com/graphql` carrying a read-only query fixed at build time, never a mutation or subscription; and
- `GET` requests for the pages of `https://api.github.com/repos/{owner}/{name}/pulls/{number}/files`.

A file read, which loads one file GitHub sent without its patch, SHALL consist only of:

- at most one `GET` of `https://api.github.com/repos/{owner}/{name}/compare/{base}...{head}`, naming the cached detail's base and head commits; and
- `GET` requests of `https://api.github.com/repos/{owner}/{name}/contents/{path}`, each naming one of the cached detail's commits as its `ref`, for a path among the cached detail's files.

A detail read or a file read SHALL be sent only for a pull request listed in the current GitHub snapshot. Its owner, repository name and number SHALL be taken from that row, never from a URL or any other value the caller supplies, and SHALL reach the query only through its GraphQL `variables`, so the query's text stays byte-identical whichever pull request it reads. A file read's commits and path SHALL be taken from the cached detail, never from the caller.

The token SHALL NOT be written to logs or diagnostic output and SHALL NOT be sent through an ambient proxy configuration. No request SHALL follow a redirect. All GitHub network activity SHALL occur only while the feature is enabled: a detail read and a file read SHALL re-check the enabled flag before each of their requests, and SHALL send nothing more once the feature is disabled. No other GitHub host SHALL be contacted.

#### Scenario: Only the official endpoint

- **WHEN** the poller issues any request
- **THEN** its URL is `https://api.github.com/graphql` and no other destination receives the token

#### Scenario: Token never logged

- **WHEN** the poller or a detail read builds and sends a request, or a request fails
- **THEN** the token value appears in no log line or diagnostic output

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made, by the poller, by a detail read or by a file read, regardless of the stored token, the environment, or the refresh interval

#### Scenario: Detail reads stay on the API host

- **WHEN** a detail read runs for pull request 42 of `acme/api`, which the GitHub snapshot lists
- **THEN** each of its requests is either the detail query sent as a `POST` to `https://api.github.com/graphql` or a `GET` of a page of `https://api.github.com/repos/acme/api/pulls/42/files`
- **AND** no other destination receives the token

#### Scenario: File reads stay on the API host

- **WHEN** a file read loads `src/huge.json` of pull request 42 of `acme/api`
- **THEN** each of its requests is a `GET` of `https://api.github.com/repos/acme/api/compare/…` naming the cached base and head commits, or of `https://api.github.com/repos/acme/api/contents/src/huge.json` naming a cached commit as its `ref`
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

### Requirement: The Terminal Frontend Does Not Render the GitHub Panel

The terminal frontend SHALL expose the GitHub enabled toggle on its Settings screen (see the *Terminal Settings Screen* requirement in the `terminal-ui` capability) but SHALL NOT start the GitHub poller and SHALL NOT render a pull-request list. The terminal frontend SHALL load a settings file carrying a `github` block without error.

#### Scenario: The terminal loads the settings block

- **WHEN** the terminal frontend starts with a settings file containing a `github` block
- **THEN** it starts successfully and makes no request to GitHub

#### Scenario: The terminal flips the shared toggle

- **WHEN** the user flips the GitHub pull-requests toggle on the terminal's Settings screen
- **THEN** `github.enabled` is written to the shared application settings
- **AND** the terminal itself still renders no pull-request list

