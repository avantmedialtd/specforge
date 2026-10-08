## MODIFIED Requirements

### Requirement: Privacy and Safety

The system SHALL send the BitBucket credential only to `https://api.bitbucket.org`, and only in the `Authorization` header of the requests named in *Authored Pull-Request Discovery* and of the pull-request viewer's detail reads and image reads (see the *BitBucket Detail Reads* and *Pull-Request Image Reads* requirements in the `pull-request-viewer` capability).

A detail read SHALL send only `GET` requests, and only for a pull request listed in the current BitBucket snapshot, with its workspace, repository and id taken from that row, never from a URL or any other value the caller supplies. It SHALL request the pull request at `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/pullrequests/{id}`, its comments and its build statuses beneath that path, and its diffstat and its diff by the links the pull request's payload names. A detail read SHALL follow a link taken from a reply's payload only when the link parses as an `https` URL whose host is exactly `api.bitbucket.org`, with no user information, no explicit port, and a path under `/2.0/`. A detail read SHALL NOT follow a redirect.

An image read, which loads the two versions of one image file, SHALL send only `GET` requests, and only for a pull request listed in the current BitBucket snapshot. Its workspace and repository SHALL be taken from that row, and its commits and paths from the cached detail, never from the caller. It SHALL request:
- the merge base of the cached detail's head and base commits at `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/merge-base/{head}..{base}`;
- each version beneath `https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/src/`, at the merge base or the head commit, for a path among the cached detail's files.

An image read SHALL follow no link taken from a payload, and SHALL NOT follow a redirect.

The token SHALL NOT be written to logs or diagnostic output, SHALL NOT be sent through an ambient proxy configuration, and all BitBucket network activity SHALL occur only while the feature is enabled: a detail read and an image read SHALL each re-check the enabled flag before each of its requests, and SHALL send nothing more once the feature is disabled.

#### Scenario: Token never logged

- **WHEN** the poller, a detail read or an image read builds and sends a request
- **THEN** the token value appears in no log line or diagnostic output

#### Scenario: Only the official endpoint

- **WHEN** the poller, a detail read or an image read issues any request
- **THEN** its host is `api.bitbucket.org` and no other destination receives the credential

#### Scenario: No request while disabled

- **WHEN** the feature is disabled
- **THEN** no request is made, by the poller, by a detail read or by an image read, regardless of the stored credentials or the refresh interval

#### Scenario: A payload link off the API is not followed

- **WHEN** a pull request's payload names its diff link on `bitbucket.org` rather than `api.bitbucket.org`, or on `api.bitbucket.org` with user information, an explicit port, or a path outside `/2.0/`
- **THEN** the detail read does not request that link
- **AND** no other destination receives the credential

#### Scenario: A detail read follows no redirect

- **WHEN** BitBucket answers one of a detail read's requests with a redirect
- **THEN** the redirect is not followed and no other destination receives the credential
- **AND** the read reports that pull request as unavailable

#### Scenario: A pull request outside the snapshot is not read

- **WHEN** `get_pull_request_detail` is invoked for a BitBucket pull request that matches no row of the current BitBucket snapshot
- **THEN** no request is sent to BitBucket

#### Scenario: Disabling stops a detail read between requests

- **WHEN** the feature is disabled while a detail read is between two of its requests
- **THEN** the read sends no further request
- **AND** its result is neither cached nor returned

#### Scenario: An image read stays on the API host

- **WHEN** an image read loads `icons/app.png` of pull request 7 of `acme/web`, which the BitBucket snapshot lists
- **THEN** each of its requests is a `GET` of `https://api.bitbucket.org/2.0/repositories/acme/web/merge-base/…` naming the cached head and base commits, or of `https://api.bitbucket.org/2.0/repositories/acme/web/src/…/icons/app.png` at the merge base or the head commit
- **AND** no other destination receives the credential

#### Scenario: An image read follows no redirect

- **WHEN** BitBucket answers an image read's `src` request with a redirect
- **THEN** the redirect is not followed and no other destination receives the credential
- **AND** the read reports the file as redirected

