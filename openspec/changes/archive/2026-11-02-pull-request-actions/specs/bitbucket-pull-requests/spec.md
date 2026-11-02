## MODIFIED Requirements

### Requirement: Credentials Are Stored Write-Only

The native desktop Settings view SHALL accept a BitBucket username (or email) and BitBucket-specific API token as one credential pair and SHALL save a newly submitted complete pair in the operating system credential store under SpecForge's BitBucket namespace. A non-empty token with an empty username SHALL be rejected without changing either store. The secure write and atomic settings update SHALL form one reported rollback-safe transaction: only the versioned secure pair identified as active by committed non-secret settings metadata is eligible, and a newly staged pair SHALL NOT become eligible until settings activates it and removes the plaintext username and token fields. Setting an empty token SHALL clear both the active secure pair and every legacy plaintext value through the same rollback-safe boundary. Any failed save, replacement or clear SHALL leave the previously resolved and stored credential state and write authority unchanged, and no operation SHALL report success while only half applied. Interrupted staging and cleanup SHALL recover as specified by *The Settings File Is Saved Atomically and in Order*. The browser transport SHALL expose no BitBucket credential setter or clearer.

Legacy settings containing a complete plaintext pair SHALL remain readable for list, detail and image reads, but that source SHALL NOT authorise a pull-request review write (see the *Current State Authorises Every Draft and Submission Boundary* requirement in the `pull-request-actions` capability). No command on any transport SHALL return either credential value. The configuration getter SHALL report only whether a complete credential is available, its active source (`environment`, `secure`, `legacy` or `none`) and its independently clearable stored source (`secure`, `legacy` or `none`), together with the enabled flag, refresh interval and panel position. Thus a complete environment override MAY be active while native Settings still reports an underlying stored pair that its clear action can remove.

Credential resolution SHALL preserve this order: a complete non-empty `BITBUCKET_USERNAME` + `BITBUCKET_API_TOKEN` environment pair, then the complete active operating-system credential-store pair named by settings, then a complete legacy plaintext pair. A partial environment pair SHALL NOT shadow either stored pair. When no source yields both values the snapshot SHALL be unauthenticated. A durable resolution for a review-action boundary SHALL recover any credential transaction and read current settings under the settings inter-process lock rather than trust this process's older in-memory secure-source metadata. Review actions SHALL accept an environment or secure-store pair only after the action capability's fingerprint, actor, snapshot, commit and decision checks succeed.

The Settings copy SHALL state that a Jira or Confluence API token is not accepted by BitBucket Cloud and where a BitBucket token is created. It SHALL name the account, workspace-membership, pull-request and other read scopes required by discovery and the pull-request viewer (see the *BitBucket Detail Reads* requirement in the `pull-request-viewer` capability). It SHALL separately name the pull-request write scope needed for comments, replies, approvals and change requests, explain that a read-only token still powers the panel and viewer but not review actions, and state that current authored-only discovery exposes Comment only.

#### Scenario: The token is never read back

- **WHEN** a frontend requests the BitBucket configuration
- **THEN** the response reports the non-secret active and stored credential sources and whether a complete pair is available
- **AND** neither username nor token value is returned

#### Scenario: Replacing the credentials

- **WHEN** no complete BitBucket environment pair is active, and the user submits a complete pair in native Settings and the complete save transaction succeeds
- **THEN** the pair is persisted securely, every legacy plaintext field is removed, and the next refresh uses it without an application restart

#### Scenario: A failed secure save grants no new authority

- **WHEN** the user submits a pair and the operating system credential store rejects or cannot persist it
- **THEN** Settings reports the failure on the credential control
- **AND** the submitted pair is not written to application settings or used for a review action

#### Scenario: An incomplete pair changes nothing

- **WHEN** the user submits a non-empty BitBucket token with an empty username
- **THEN** Settings reports that the pair is incomplete
- **AND** neither the secure store nor settings changes

#### Scenario: A failed settings commit preserves the prior pair

- **WHEN** secure staging succeeds but removing legacy plaintext or activating the new secure pair cannot be persisted
- **THEN** Settings reports the save failure and the previous pair remains the resolved credential
- **AND** the staged pair grants no review authority

#### Scenario: A staged pair remains ineligible after interruption

- **WHEN** the process exits after securely staging a replacement pair but before settings activates it
- **THEN** the next durable resolution recovers the transaction and resolves the previously committed source
- **AND** the staged pair grants no read or review authority

#### Scenario: Clearing the token

- **WHEN** the user submits an empty token and no complete environment pair is set
- **THEN** the secure pair and legacy plaintext fields are removed
- **AND** the next refresh reports the unauthenticated state

#### Scenario: A failed clear restores both stores

- **WHEN** clearing one stored pair resource succeeds but clearing the other resource or committing settings fails
- **THEN** Settings reports the clear failure and restores the previous stored state
- **AND** the previously resolved pair and write authority remain in effect

#### Scenario: Environment variables override the stored pair

- **WHEN** `BITBUCKET_USERNAME` and `BITBUCKET_API_TOKEN` are both set to non-empty values
- **THEN** the poller authenticates with the environment pair regardless of what is securely or historically stored

#### Scenario: A partial environment does not override

- **WHEN** only one of the two environment variables is set
- **THEN** the secure pair is used when complete, otherwise the complete legacy pair is used

#### Scenario: Secure storage precedes legacy plaintext

- **WHEN** no complete environment pair is set and both a secure pair and a legacy plaintext pair exist
- **THEN** the secure pair is used
- **AND** the legacy plaintext pair cannot authorise a review write

#### Scenario: An environment override does not hide a stored pair

- **WHEN** a complete environment pair is active while a secure or legacy pair is also stored
- **THEN** the configuration reports `environment` as the active source and the applicable non-secret stored source separately
- **AND** native Settings can clear the stored pair without changing the process environment

#### Scenario: A token created as the copy says reads an opened pull request

- **WHEN** a BitBucket token is created with only the read scopes the Settings copy names, and the user opens one of the pull requests the panel lists
- **THEN** the panel lists the account's authored pull requests and reads the opened pull request without an HTTP 403
- **AND** a later review mutation without the separately documented write scope is reported as a permission failure and is not retried

### Requirement: Authored Pull-Request Discovery

While enabled and authenticated, the system SHALL list the pull requests authored by the configured account across every workspace it belongs to using only HTTP GET requests: `GET /2.0/user` to resolve the account, `GET /2.0/user/workspaces` to enumerate workspaces, and `GET /2.0/workspaces/{workspace}/pullrequests/{account}` per workspace. The account identifier SHALL be the account's `uuid` when present and SHALL otherwise be its `account_id`. Private snapshot context SHALL retain both stable values that the user reply supplies, its display name and the non-secret fingerprint of the resolved credential under which the refresh ran for action fencing; it SHALL expose no fingerprint and SHALL NOT infer actor identity from the configured username. The system SHALL NOT call the removed `GET /2.0/pullrequests/{selected_user}` endpoint.

Each per-workspace request SHALL carry `state=OPEN`, `sort=-updated_on`, a page length of 50, and the `fields` partial-response parameter adding `participants` and `reviewers` to the list values, with every `+` in that parameter percent-encoded as `%2B`. Only the first page of each workspace's pull requests SHALL be fetched. The workspace listing SHALL request a page length of 100 and SHALL follow the page's `next` link while one is present, up to a fixed bound of five pages, following only links that point back at the official API. For an account in at most 100 workspaces the number of requests per refresh is therefore

$$\text{requests} = 2 + W$$

for an account in $$W$$ workspaces.

A workspace whose request answers HTTP 403 or 404 SHALL be skipped and named by slug in the snapshot; the remaining workspaces SHALL still be listed. Results from all workspaces SHALL be merged and ordered by `updated_on`, most recent first. Because every discovered row is authored by the authenticated actor, each row SHALL expose Comment as its only review decision; approval and request-changes capability SHALL require a future reviewer-discovery source and SHALL NOT be inferred from participant fields.

#### Scenario: Pull requests are listed from every workspace

- **WHEN** the account belongs to workspaces `W1` and `W2`
- **THEN** the account is resolved via `GET /2.0/user`, the workspaces via `GET /2.0/user/workspaces`, and `GET /2.0/workspaces/{ws}/pullrequests/{account}` is requested for both with `state=OPEN`
- **AND** the snapshot contains the open pull requests authored by the account in both workspaces

#### Scenario: Stable actor identity is retained

- **WHEN** `GET /2.0/user` returns `account_id`, `uuid` and display name
- **THEN** the snapshot retains the stable ids for action fencing
- **AND** changing only the configured username does not substitute a different actor identity

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

#### Scenario: Authored rows do not advertise reviewer decisions

- **WHEN** an authored row's participants include reviewers who approved or requested changes
- **THEN** the row still exposes Comment only to the authenticated author
- **AND** it does not infer that the author is authorised to approve or request changes

### Requirement: Privacy and Safety

The system SHALL send the BitBucket credential only to `https://api.bitbucket.org`, and only in the `Authorization` header of:

- the requests named in *Authored Pull-Request Discovery*;
- the pull-request viewer's detail and image reads (see the *BitBucket Detail Reads* and *Pull-Request Image Reads* requirements in the `pull-request-viewer` capability); and
- the bounded reads and writes required by *BitBucket Submission Is Sequential and Durably Partial* in the `pull-request-actions` capability.

A detail read SHALL send only `GET` requests, and only for a pull request listed in the current BitBucket snapshot, with its workspace, repository and id taken from that row, never from a URL or any other value the caller supplies. It SHALL request the pull request at `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/pullrequests/{id}`, its comments and its build statuses beneath that path, and its diffstat and its diff by the links the pull request's payload names. It SHALL NOT request the diffstat or the diff when it reuses the cached detail's files, which it does only when the payload reports the cached commits and names the cached links, each naming those commits as its diff spec (see the *Detail Re-reads Reuse Unchanged Files* requirement in the `pull-request-viewer` capability). A detail read SHALL follow a link taken from a reply's payload only when the link parses as an `https` URL whose host is exactly `api.bitbucket.org`, with no user information, no explicit port, and a path under `/2.0/`. A detail read SHALL NOT follow a redirect.

An image read, which loads the two versions of one image file, SHALL send only `GET` requests, and only for a pull request listed in the current BitBucket snapshot. Its workspace and repository SHALL be taken from that row, and its commits and paths from the cached detail, never from the caller. It SHALL request:
- the merge base of the cached detail's head and base commits at `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/merge-base/{head}..{base}`;
- each version beneath `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/src/`, at the merge base or the head commit, for a path among the cached detail's files.

An image read SHALL follow no link taken from a payload, and SHALL NOT follow a redirect. An image read of a file whose old or new path has a `.` or `..` segment SHALL be unavailable, and SHALL send nothing, since the API host would resolve the segment to another of its paths.

A review operation SHALL send only the pull-request and comments reads needed for current authorisation, immediate pre/post-write commit checks and reconciliation, content `POST`s beneath the listed pull request's comments collection for general comments, single-line inline comments and replies, and, only when current capabilities permit one, the pull request's `approve` or `request-changes` `POST`. Workspace, repository and pull-request id SHALL come from the current snapshot; commits, inline path/side/line and reply target SHALL come from the currently authorised detail and frozen draft. One comment-reconciliation pass SHALL read at most ten pages of 100 comments and at most 10 MiB for any one response body; reaching either bound SHALL pause as uncertain rather than prove a marker absent. It SHALL send no merge, decline, close, branch, repository-content, task, thread-resolution, thread-reopening or unrelated-comment mutation.

The token SHALL NOT be written to logs, journals, events or diagnostic output and SHALL NOT be sent through an ambient proxy configuration. No request SHALL follow a redirect. All BitBucket network activity SHALL occur only while the feature is enabled: a detail read and image read SHALL re-check the enabled flag and credential generation before every request, and a review operation SHALL additionally re-check its current listed row, trusted ids, freshly resolved credential fingerprint and stable actor against its operation stamp as the action capability requires. Each SHALL send nothing more once its applicable check fails.

#### Scenario: Token never logged

- **WHEN** the poller, a pull-request read or a review operation builds and sends a request
- **THEN** the token value appears in no log line, journal, event or diagnostic output

#### Scenario: Only the official endpoint

- **WHEN** the poller, a pull-request read or a review operation issues any request
- **THEN** its host is `api.bitbucket.org` and no other destination receives the credential

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made by the poller, a detail read, an image read or a review operation, regardless of the credential source or refresh interval

#### Scenario: A payload link off the API is not followed

- **WHEN** a pull request's payload names its diff link on `bitbucket.org` rather than `api.bitbucket.org`, or on `api.bitbucket.org` with user information, an explicit port, or a path outside `/2.0/`
- **THEN** the detail read does not request that link
- **AND** no other destination receives the credential

#### Scenario: A detail read follows no redirect

- **WHEN** BitBucket answers one of a detail read's requests with a redirect
- **THEN** the redirect is not followed and no other destination receives the credential
- **AND** the read reports that pull request as unavailable

#### Scenario: A reusing read sends nothing to the diff links

- **WHEN** a detail read that is not a manual refresh, under the credential generation the cached detail was stored under, has a payload that reports the cached detail's commits and names the cached `links.diff` and `links.diffstat`, each naming those commits as BitBucket writes them (`…:{source}%0D{destination}`, compared case-insensitively)
- **THEN** no request is sent to either link
- **AND** every request the read does send is a `GET` to `api.bitbucket.org` beneath the pull request's path

#### Scenario: A pull request outside the snapshot is not read

- **WHEN** `get_pull_request_detail` is invoked for a BitBucket pull request that matches no row of the current BitBucket snapshot
- **THEN** no request is sent to BitBucket

#### Scenario: A pull request outside the snapshot is not reviewed

- **WHEN** a review command names a BitBucket pull request that matches no row of the current BitBucket snapshot
- **THEN** no request is sent to BitBucket

#### Scenario: Disabling stops a detail read between requests

- **WHEN** the feature is disabled while a detail read is between two of its requests
- **THEN** the read sends no further request
- **AND** its result is neither cached nor returned

#### Scenario: Disabling stops a review operation between requests

- **WHEN** the feature is disabled while a review operation is between two requests
- **THEN** it sends no further request
- **AND** it retains its confirmed ledger and pauses

#### Scenario: An image read stays on the API host

- **WHEN** an image read loads `icons/app.png` of pull request 7 of `acme/web`, which the BitBucket snapshot lists
- **THEN** each of its requests is a `GET` of `https://api.bitbucket.org/2.0/repositories/acme/web/merge-base/…` naming the cached head and base commits, or of `https://api.bitbucket.org/2.0/repositories/acme/web/src/…/icons/app.png` at the merge base or the head commit
- **AND** no other destination receives the credential

#### Scenario: A path that would leave its commit is not read

- **WHEN** an image read is asked for a file that the cached detail lists at `img/../../user`
- **THEN** the read is unavailable and no request is sent

#### Scenario: An image read follows no redirect

- **WHEN** BitBucket answers an image read's `src` request with a redirect
- **THEN** the redirect is not followed and no other destination receives the credential
- **AND** the read reports the file as redirected

#### Scenario: Review mutations stay under the listed pull request

- **WHEN** a BitBucket review operation posts a comment, reply or available decision for pull request 7 of `acme/web`
- **THEN** the request is a `POST` beneath `https://api.bitbucket.org/2.0/repositories/acme/web/pullrequests/7`
- **AND** no request can merge, decline, close, resolve or reopen a thread, or change repository content
