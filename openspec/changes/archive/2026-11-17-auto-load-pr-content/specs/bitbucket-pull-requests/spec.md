## MODIFIED Requirements

### Requirement: Privacy and Safety

The system SHALL send the BitBucket credential only to `https://api.bitbucket.org`, and only in the `Authorization` header of:

- the requests named in *Authored Pull-Request Discovery*;
- the pull-request viewer's detail and changed-file image reads (see the *BitBucket Detail Reads* and *Pull-Request Image Reads* requirements in the `pull-request-viewer` capability); and
- the bounded reads and writes required by *BitBucket Submission Is Sequential and Durably Partial* in the `pull-request-actions` capability.

A detail read SHALL send only `GET` requests, and only for a pull request listed in the current BitBucket snapshot, with its workspace, repository and id taken from that row, never from a URL or any other value the caller supplies. It SHALL request the pull request at `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/pullrequests/{id}`, its comments and its build statuses beneath that path, and its diffstat and its diff by the links the pull request's payload names. It SHALL NOT request the diffstat or the diff when it reuses the cached detail's files, which it does only when the payload reports the cached commits and names the cached links, each naming those commits as its diff spec (see the *Detail Re-reads Reuse Unchanged Files* requirement in the `pull-request-viewer` capability). A detail read SHALL follow a link taken from a reply's payload only when the link parses as an `https` URL whose host is exactly `api.bitbucket.org`, with no user information, no explicit port, and a path under `/2.0/`. A detail read SHALL NOT follow a redirect.

A changed-file image read, which loads the two versions of one image file, SHALL send only `GET` requests, and only for a pull request listed in the current BitBucket snapshot. Its workspace and repository SHALL be taken from that row, and its commits and paths from the cached detail, never from the caller. It SHALL request:
- the merge base of the cached detail's head and base commits at `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/merge-base/{head}..{base}`;
- each version beneath `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/src/`, at the merge base or the head commit, for a path among the cached detail's files.

A changed-file image read SHALL follow no link taken from a payload, and SHALL NOT follow a redirect. A changed-file image read of a file whose old or new path has a `.` or `..` segment SHALL be unavailable, and SHALL send nothing, since the API host would resolve the segment to another of its paths.

A review operation SHALL send only the pull-request and comments reads needed for current authorisation, immediate pre/post-write commit checks and reconciliation, content `POST`s beneath the listed pull request's comments collection for general comments, single-line inline comments and replies, and, only when current capabilities permit one, the pull request's `approve` or `request-changes` `POST`. Workspace, repository and pull-request id SHALL come from the current snapshot; commits, inline path/side/line and reply target SHALL come from the currently authorised detail and frozen draft. One comment-reconciliation pass SHALL read at most ten pages of 100 comments and at most 10 MiB for any one response body; reaching either bound SHALL pause as uncertain rather than prove a marker absent. It SHALL send no merge, decline, close, branch, repository-content, task, thread-resolution, thread-reopening or unrelated-comment mutation.

The token SHALL NOT be written to logs, journals, events or diagnostic output and SHALL NOT be sent through an ambient proxy configuration. No authenticated operation above SHALL follow a redirect. All BitBucket integration activity SHALL occur only while the feature is enabled: a detail read and changed-file image read SHALL re-check the enabled flag and credential generation before every request, and a review operation SHALL additionally re-check its current listed row, trusted ids, freshly resolved credential fingerprint and stable actor against its operation stamp as the action capability requires. Each SHALL send nothing more once its applicable check fails.

Embedded description/comment images SHALL be a separate, credentialless content operation governed by the *Embedded Image Reads Are Authorised by Published Content* and *Embedded Image Transport Cannot Reach Private Networks or Carry Credentials* requirements in the `pr-content-loading` capability. That operation SHALL be allowed to contact its authorised public HTTPS source and bounded independently checked redirect targets while the provider and image preference are enabled, but SHALL never carry the BitBucket credential or borrow the authenticated transport. It SHALL not broaden any authenticated endpoint, redirect or provider budget rule.

#### Scenario: Token never logged

- **WHEN** the poller, a pull-request read or a review operation builds and sends a request
- **THEN** the token value appears in no log line, journal, event or diagnostic output

#### Scenario: Only the official endpoint

- **WHEN** the poller, an authenticated pull-request read or a review operation issues any request
- **THEN** its host is `api.bitbucket.org` and no other destination receives the credential

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made by the poller, a detail read, a changed-file image read, an embedded-image read or a review operation, regardless of the credential source or refresh interval

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

- **WHEN** a changed-file image read loads `icons/app.png` of pull request 7 of `acme/web`, which the BitBucket snapshot lists
- **THEN** each of its requests is a `GET` of `https://api.bitbucket.org/2.0/repositories/acme/web/merge-base/…` naming the cached head and base commits, or of `https://api.bitbucket.org/2.0/repositories/acme/web/src/…/icons/app.png` at the merge base or the head commit
- **AND** no other destination receives the credential

#### Scenario: A path that would leave its commit is not read

- **WHEN** a changed-file image read is asked for a file that the cached detail lists at `img/../../user`
- **THEN** the read is unavailable and no request is sent

#### Scenario: An image read follows no redirect

- **WHEN** BitBucket answers a changed-file image read's `src` request with a redirect
- **THEN** the redirect is not followed and no other destination receives the credential
- **AND** the read reports the file as redirected

#### Scenario: Review mutations stay under the listed pull request

- **WHEN** a BitBucket review operation posts a comment, reply or available decision for pull request 7 of `acme/web`
- **THEN** the request is a `POST` beneath `https://api.bitbucket.org/2.0/repositories/acme/web/pullrequests/7`
- **AND** no request can merge, decline, close, resolve or reopen a thread, or change repository content

#### Scenario: A screenshot host receives no BitBucket authority

- **WHEN** an enabled embedded-image read fetches a supported screenshot from a public HTTPS host named in a current published PR comment
- **THEN** that host receives a credentialless request through the separate checked transport
- **AND** changed-file reads still send the credential only to `api.bitbucket.org`
