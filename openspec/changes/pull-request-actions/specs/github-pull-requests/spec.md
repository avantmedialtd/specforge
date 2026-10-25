## MODIFIED Requirements

### Requirement: The GitHub Token Is Stored Write-Only

The native desktop Settings view SHALL accept a single GitHub token and SHALL save a newly submitted non-empty token in the operating system credential store under SpecForge's GitHub namespace. The secure write and atomic settings update SHALL form one reported rollback-safe transaction: only the versioned secure entry identified as active by committed non-secret settings metadata is eligible, and a newly staged entry SHALL NOT become eligible until settings activates it and removes the plaintext token field. Setting an empty token SHALL clear both the active secure entry and any legacy plaintext value through the same rollback-safe boundary. Any failed save, replacement or clear SHALL leave the previously resolved and stored credential state and write authority unchanged, and no operation SHALL report success while only half applied. Interrupted staging and cleanup SHALL recover as specified by *The Settings File Is Saved Atomically and in Order*. The browser transport SHALL expose no token setter or clearer.

Legacy settings containing a plaintext token SHALL remain readable for list, detail, file and image reads, but that source SHALL NOT authorise a pull-request review write (see the *Current State Authorises Every Draft and Submission Boundary* requirement in the `pull-request-actions` capability). No command on any transport SHALL return a token. The configuration getter SHALL report only whether a credential is available, its active source (`environment`, `secure`, `legacy` or `none`) and its independently clearable stored source (`secure`, `legacy` or `none`), together with the enabled flag, refresh interval and panel position. Thus an environment override MAY be active while native Settings still reports an underlying stored entry that its clear action can remove.

Credential resolution SHALL preserve this order, skipping an absent or empty source: `GH_TOKEN`, then `GITHUB_TOKEN`, then the active operating-system credential-store entry named by settings, then the legacy plaintext setting. When none yields a token the snapshot SHALL be unauthenticated. A durable resolution for a review-action boundary SHALL recover any credential transaction and read current settings under the settings inter-process lock rather than trust this process's older in-memory secure-source metadata. Review actions SHALL accept an environment or secure-store credential only after the action capability's fingerprint, actor, snapshot, commit and decision checks succeed.

The Settings copy SHALL state that a classic token needs the `repo` scope to see private repositories and that this scope permits the bounded review writes SpecForge performs after confirmation; that a fine-grained token needs read access required by the pull-request viewer and read/write pull-request access to submit reviews; that a read-only token can still power the panel and viewer but not review actions; that a token must be authorised for each organisation enforcing single sign-on; that `gh auth token` prints a token the GitHub CLI already holds; and where a GitHub token is created.

#### Scenario: The token is never read back

- **WHEN** a frontend requests the GitHub configuration
- **THEN** the response reports the non-secret active and stored credential sources and whether a token is available
- **AND** the token value is absent from the response

#### Scenario: Replacing the token stores it securely

- **WHEN** no GitHub token environment variable is active, and the user submits a token in native Settings and the complete save transaction succeeds
- **THEN** the secure entry is persisted, any legacy plaintext field is removed, and the next refresh uses it without an application restart

#### Scenario: A failed secure save grants no new authority

- **WHEN** the user submits a token and the operating system credential store rejects or cannot persist it
- **THEN** Settings reports the failure on the token control
- **AND** the submitted token is not written to application settings or used for a review action

#### Scenario: A failed settings commit preserves the prior credential

- **WHEN** secure staging succeeds but removing legacy plaintext or activating the new secure entry cannot be persisted
- **THEN** Settings reports the save failure and the previous credential remains the resolved credential
- **AND** the staged token grants no review authority

#### Scenario: A staged token remains ineligible after interruption

- **WHEN** the process exits after securely staging a replacement token but before settings activates it
- **THEN** the next durable resolution recovers the transaction and resolves the previously committed source
- **AND** the staged token grants no read or review authority

#### Scenario: Clearing the token clears both stores

- **WHEN** the user submits an empty token and neither environment variable is set
- **THEN** the secure entry and legacy plaintext field are removed
- **AND** the next refresh reports the unauthenticated state

#### Scenario: A failed clear restores both stores

- **WHEN** clearing one stored token resource succeeds but clearing the other resource or committing settings fails
- **THEN** Settings reports the clear failure and restores the previous stored state
- **AND** the previously resolved credential and write authority remain in effect

#### Scenario: GH_TOKEN takes precedence

- **WHEN** `GH_TOKEN` and `GITHUB_TOKEN` are both set to non-empty values and a token is held in the credential store or legacy settings
- **THEN** the poller authenticates with `GH_TOKEN`

#### Scenario: GITHUB_TOKEN is the fallback

- **WHEN** `GH_TOKEN` is absent or empty and `GITHUB_TOKEN` is set
- **THEN** the poller authenticates with `GITHUB_TOKEN` regardless of what is securely or historically stored

#### Scenario: Secure storage precedes legacy plaintext

- **WHEN** neither environment variable is set and both a secure entry and a legacy plaintext token exist
- **THEN** the secure entry is used
- **AND** the legacy plaintext token cannot authorise a review write

#### Scenario: An environment override does not hide a stored token

- **WHEN** `GH_TOKEN` is active while a secure or legacy token is also stored
- **THEN** the configuration reports `environment` as the active source and the applicable non-secret stored source separately
- **AND** native Settings can clear the stored token without changing the process environment

### Requirement: One Constant Read-Only Query

While enabled and holding a token, each refresh SHALL issue exactly one HTTP request: a `POST` to `https://api.github.com/graphql` whose body carries a single GraphQL query fixed at build time. The query SHALL be a read-only operation — never a mutation or subscription — and no value supplied at runtime SHALL be interpolated into it.

The query SHALL list two sets of open pull requests:

- **authored** — the open pull requests authored by the token's account, across every repository the token can read, most recently updated first; and
- **review requested** — the open pull requests on which a review is requested from the token's account, directly or through a team it belongs to, most recently updated first.

The query SHALL also read the viewer's stable node id and login, and each listed pull request's node id and author's stable node id and login. Private snapshot context SHALL retain those values and the non-secret fingerprint of the resolved credential under which the refresh ran for action authorisation, but SHALL expose no credential or fingerprint through the row or public snapshot. A response without a stable viewer id SHALL remain usable for read-only rows but SHALL give those rows no review-action capability.

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

#### Scenario: Stable identities accompany an actionable row

- **WHEN** the poll query returns a viewer and a review-requested pull request
- **THEN** the snapshot retains the viewer node id, pull-request node id and author node id
- **AND** later action authorisation does not infer identity from display login alone

#### Scenario: Missing viewer identity leaves reads available

- **WHEN** the poll response can be parsed into rows but carries no stable viewer id
- **THEN** those rows remain available to the panel and pull-request viewer
- **AND** every review-action capability on them is unavailable

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

### Requirement: GitHub Privacy and Safety

The system SHALL send the GitHub token only to `https://api.github.com`, and only in the `Authorization` header of:

- the poller's request named in *One Constant Read-Only Query*;
- the pull-request viewer's detail, file and image reads (see the *GitHub Detail Reads* and *Pull-Request Image Reads* requirements in the `pull-request-viewer` capability); and
- the bounded query and mutation operations required by *GitHub Submission Owns a Commit-Pinned Pending Review* in the `pull-request-actions` capability.

A detail read SHALL consist only of:

- a `POST` to `https://api.github.com/graphql` carrying a read-only query fixed at build time, never a mutation or subscription; and
- `GET` requests for the pages of `https://api.github.com/repos/{owner}/{name}/pulls/{number}/files`.

A file read, which loads one file GitHub sent without its patch, and an image read, which loads the two versions of one image file, SHALL each consist only of:

- at most one `GET` of `https://api.github.com/repos/{owner}/{name}/compare/{base}...{head}`, naming the cached detail's base and head commits; and
- `GET` requests of `https://api.github.com/repos/{owner}/{name}/contents/{path}`, each naming as its `ref` either the cached detail's head commit or the merge base that compare returned for it, for a path among the cached detail's files.

A review operation SHALL send only fixed-at-build-time GraphQL queries and mutations needed to read/create an owned pending review, add its single-line comments and replies, update this operation's marker-bearing bodies, submit that review, or delete that same marked pending review after explicit abandonment. Runtime values SHALL travel only as GraphQL variables. One reconciliation pass SHALL read at most ten provider pages in total across the pending review's relevant connections, requesting at most 100 objects per page, and at most 10 MiB for any one response body; reaching either bound SHALL pause as uncertain rather than prove a marker absent. It SHALL send no merge, close, branch, repository-content, reaction, thread-resolution, thread-reopening or unrelated-review mutation.

A detail read, a file read, an image read or a review operation SHALL be sent only for a pull request listed in the current GitHub snapshot. Its owner, repository name, number and node id SHALL be taken from that row or a currently authorised detail, never from a URL or any other untrusted value the caller supplies. Read-query values SHALL reach GraphQL only through variables, so query text stays byte-identical whichever pull request it reads. A file read's or image read's commits and path SHALL be taken from the cached detail, never from the caller. A file read or image read of a file whose old or new path has a `.` or `..` segment SHALL be unavailable, and SHALL send nothing, since the API host would resolve the segment to another of its paths. Review values SHALL be fenced as the `pull-request-actions` capability requires.

The token SHALL NOT be written to logs or diagnostic output and SHALL NOT be sent through an ambient proxy configuration. No request SHALL follow a redirect. All GitHub network activity SHALL occur only while the feature is enabled: a detail read, file read and image read SHALL re-check the enabled flag and credential generation before each request, and a review operation SHALL additionally re-check its current listed row, trusted ids, freshly resolved credential fingerprint and stable actor against its operation stamp as the action capability requires. Each SHALL send nothing more once its applicable check fails. No other GitHub host SHALL be contacted.

#### Scenario: Only the official endpoint

- **WHEN** the poller, a pull-request read or a review operation issues any request
- **THEN** its host is `api.github.com` and no other destination receives the token

#### Scenario: Token never logged

- **WHEN** the poller, a pull-request read or a review operation builds and sends a request, or a request fails
- **THEN** the token value appears in no log line, journal, event or diagnostic output

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made by the poller, a detail read, a file read, an image read or a review operation, regardless of the credential source or refresh interval

#### Scenario: Detail reads stay on the API host

- **WHEN** a detail read runs for pull request 42 of `acme/api`, which the GitHub snapshot lists
- **THEN** each of its requests is either the detail query sent as a `POST` to `https://api.github.com/graphql` or a `GET` of a page of `https://api.github.com/repos/acme/api/pulls/42/files`
- **AND** no other destination receives the token

#### Scenario: File reads stay on the API host

- **WHEN** a file read loads `src/huge.json` of pull request 42 of `acme/api`
- **THEN** each of its requests is a `GET` of `https://api.github.com/repos/acme/api/compare/…` naming the cached base and head commits, or of `https://api.github.com/repos/acme/api/contents/src/huge.json` naming the cached head commit or the merge base as its `ref`
- **AND** no other destination receives the token

#### Scenario: Image reads stay on the API host

- **WHEN** an image read loads `icons/app.png` of pull request 42 of `acme/api`
- **THEN** each of its requests is a `GET` of `https://api.github.com/repos/acme/api/compare/…` naming the cached base and head commits, or of `https://api.github.com/repos/acme/api/contents/icons/app.png` naming the cached head commit or the merge base as its `ref`
- **AND** no other destination receives the token

#### Scenario: A path that would leave its commit is not read

- **WHEN** a file read or an image read is asked for a file that the cached detail lists at `img/../../user`
- **THEN** the read is unavailable and no request is sent

#### Scenario: A pull request outside the snapshot is not read

- **WHEN** `get_pull_request_detail` is invoked for a GitHub pull request that matches no row of the current GitHub snapshot
- **THEN** no request is sent to GitHub

#### Scenario: A pull request outside the snapshot is not reviewed

- **WHEN** a review command names a GitHub pull request that matches no row of the current GitHub snapshot
- **THEN** no request is sent to GitHub

#### Scenario: A detail read follows no redirect

- **WHEN** GitHub answers a detail read's files request with a redirect
- **THEN** the redirect is not followed and the token is sent nowhere else
- **AND** the read reports that pull request as unavailable

#### Scenario: Disabling stops a detail read between requests

- **WHEN** the feature is disabled while a detail read is between two of its requests
- **THEN** the read sends no further request
- **AND** its result is neither cached nor returned

#### Scenario: Disabling stops a review operation between requests

- **WHEN** the feature is disabled while a review operation is between two requests
- **THEN** it sends no further request
- **AND** it retains its confirmed ledger and pauses

#### Scenario: Review mutations are bounded

- **WHEN** every GraphQL mutation available to a GitHub review operation is inspected
- **THEN** each acts only on the operation's owned pending review, its single-line comments or its replies
- **AND** no mutation can merge, close, change repository content, resolve or reopen a thread, or alter an unrelated review
