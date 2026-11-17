## MODIFIED Requirements

### Requirement: GitHub Privacy and Safety

The system SHALL send the GitHub token only to `https://api.github.com`, and only in the `Authorization` header of:

- the poller's request named in *One Constant Read-Only Query*;
- the pull-request viewer's detail, file and changed-file image reads (see the *GitHub Detail Reads* and *Pull-Request Image Reads* requirements in the `pull-request-viewer` capability); and
- the bounded query and mutation operations required by *GitHub Submission Owns a Commit-Pinned Pending Review* in the `pull-request-actions` capability.

A detail read SHALL consist only of:

- a `POST` to `https://api.github.com/graphql` carrying a read-only query fixed at build time, never a mutation or subscription; and
- `GET` requests for the pages of `https://api.github.com/repos/{owner}/{name}/pulls/{number}/files`.

A file read, which loads one file GitHub sent without its patch, and a changed-file image read, which loads the two versions of one image file, SHALL each consist only of:

- at most one `GET` of `https://api.github.com/repos/{owner}/{name}/compare/{base}...{head}`, naming the cached detail's base and head commits; and
- `GET` requests of `https://api.github.com/repos/{owner}/{name}/contents/{path}`, each naming as its `ref` either the cached detail's head commit or the merge base that compare returned for it, for a path among the cached detail's files.

A review operation SHALL send only fixed-at-build-time GraphQL queries and mutations needed to read/create an owned pending review, add its single-line comments and replies, update this operation's marker-bearing bodies, submit that review, or delete that same marked pending review after explicit abandonment. Runtime values SHALL travel only as GraphQL variables. One reconciliation pass SHALL read at most ten provider pages in total across the pending review's relevant connections, requesting at most 100 objects per page, and at most 10 MiB for any one response body; reaching either bound SHALL pause as uncertain rather than prove a marker absent. It SHALL send no merge, close, branch, repository-content, reaction, thread-resolution, thread-reopening or unrelated-review mutation.

A detail read, a file read, a changed-file image read or a review operation SHALL be sent only for a pull request listed in the current GitHub snapshot. Its owner, repository name, number and node id SHALL be taken from that row or a currently authorised detail, never from a URL or any other untrusted value the caller supplies. Read-query values SHALL reach GraphQL only through variables, so query text stays byte-identical whichever pull request it reads. A file read's or changed-file image read's commits and path SHALL be taken from the cached detail, never from the caller. A file read or changed-file image read of a file whose old or new path has a `.` or `..` segment SHALL be unavailable, and SHALL send nothing, since the API host would resolve the segment to another of its paths. Review values SHALL be fenced as the `pull-request-actions` capability requires.

The token SHALL NOT be written to logs or diagnostic output and SHALL NOT be sent through an ambient proxy configuration. No authenticated operation above SHALL follow a redirect. All GitHub integration activity SHALL occur only while the feature is enabled: a detail read, file read and changed-file image read SHALL re-check the enabled flag and credential generation before each request, and a review operation SHALL additionally re-check its current listed row, trusted ids, freshly resolved credential fingerprint and stable actor against its operation stamp as the action capability requires. Each SHALL send nothing more once its applicable check fails. These authenticated operations SHALL contact no GitHub host other than `api.github.com`.

Embedded description/comment images SHALL be a separate, credentialless content operation governed by the *Embedded Image Reads Are Authorised by Published Content* and *Embedded Image Transport Cannot Reach Private Networks or Carry Credentials* requirements in the `pr-content-loading` capability. That operation SHALL be allowed to contact its authorised public HTTPS source and bounded independently checked redirect targets while the provider and image preference are enabled, but SHALL never carry the GitHub token or borrow the authenticated transport. It SHALL not broaden any endpoint, redirect or budget rule of the authenticated reads above.

#### Scenario: Only the official endpoint

- **WHEN** the poller, an authenticated pull-request read or a review operation issues any request
- **THEN** its host is `api.github.com` and no other destination receives the token

#### Scenario: Token never logged

- **WHEN** the poller, a pull-request read or a review operation builds and sends a request, or a request fails
- **THEN** the token value appears in no log line, journal, event or diagnostic output

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made by the poller, a detail read, a file read, a changed-file image read, an embedded-image read or a review operation, regardless of the credential source or refresh interval

#### Scenario: Detail reads stay on the API host

- **WHEN** a detail read runs for pull request 42 of `acme/api`, which the GitHub snapshot lists
- **THEN** each of its requests is either the detail query sent as a `POST` to `https://api.github.com/graphql` or a `GET` of a page of `https://api.github.com/repos/acme/api/pulls/42/files`
- **AND** no other destination receives the token

#### Scenario: File reads stay on the API host

- **WHEN** a file read loads `src/huge.json` of pull request 42 of `acme/api`
- **THEN** each of its requests is a `GET` of `https://api.github.com/repos/acme/api/compare/…` naming the cached base and head commits, or of `https://api.github.com/repos/acme/api/contents/src/huge.json` naming the cached head commit or the merge base as its `ref`
- **AND** no other destination receives the token

#### Scenario: Image reads stay on the API host

- **WHEN** a changed-file image read loads `icons/app.png` of pull request 42 of `acme/api`
- **THEN** each of its requests is a `GET` of `https://api.github.com/repos/acme/api/compare/…` naming the cached base and head commits, or of `https://api.github.com/repos/acme/api/contents/icons/app.png` naming the cached head commit or the merge base as its `ref`
- **AND** no other destination receives the token

#### Scenario: A path that would leave its commit is not read

- **WHEN** a file read or a changed-file image read is asked for a file that the cached detail lists at `img/../../user`
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

#### Scenario: A screenshot host receives no GitHub authority

- **WHEN** an enabled embedded-image read fetches a supported screenshot from a public HTTPS host named in a current published PR comment
- **THEN** that host receives a credentialless request through the separate checked transport
- **AND** changed-file reads still send the token only to `api.github.com`
