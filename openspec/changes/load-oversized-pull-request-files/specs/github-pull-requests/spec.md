## MODIFIED Requirements

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
