## ADDED Requirements

### Requirement: Unavailable and Stale Snapshots Carry a Reason

When a refresh leaves the snapshot unavailable or stale, the snapshot SHALL carry the reason, taken from this closed set:

- `certificateUntrusted`: the server's certificate chain was rejected (unknown issuer, bad signature, wrong name, expired or not yet valid);
- `tls`: any other TLS failure;
- `dns`: the host name did not resolve;
- `connect`: the connection was refused, reset or unreachable;
- `timeout`: the request exceeded its deadline;
- `httpStatus`, with the status code: a non-success reply that the *Polling With Caching and Backoff* requirement does not map to the unauthenticated state or to a rate limit;
- `unexpectedResponse`: a reply whose body could not be parsed;
- `rateLimited`: an HTTP 429, or the shared deadline holding the poller.

An ok, unauthenticated or disabled snapshot SHALL carry no reason. The reason SHALL be part of the snapshot's identity for the *A refresh that changes nothing is silent* rule, so a change of reason is announced. The reason SHALL carry no text taken from a reply, an error message or a request.

When the snapshot is unavailable, the pull-request panel's unavailable line SHALL state the reason in words. For `certificateUntrusted`, it SHALL say that this machine does not trust the server's certificate and point to the system trust store or `SSL_CERT_FILE`. A stale snapshot SHALL make its reason visible on request, for example in the header's tooltip. An unavailable snapshot without a reason SHALL keep today's line.

#### Scenario: An untrusted certificate is named

- **WHEN** a refresh with no previous rows fails because the server's certificate chain is not trusted
- **THEN** the snapshot is unavailable with reason `certificateUntrusted`
- **AND** the panel's unavailable line says this machine does not trust the server's certificate

#### Scenario: A server error carries its status

- **WHEN** a refresh with no previous rows is answered HTTP 503 by `GET /2.0/user`
- **THEN** the snapshot is unavailable with reason `httpStatus` and status 503

#### Scenario: A stale snapshot keeps its rows and gains a reason

- **WHEN** a refresh times out while a previous ok snapshot with rows exists
- **THEN** the rows are kept and marked stale, with reason `timeout`
- **AND** the reason is shown on request without hiding the rows

#### Scenario: A changed reason is announced

- **WHEN** the snapshot is unavailable with reason `dns` and the next refresh fails with `certificateUntrusted`
- **THEN** a `bitbucket-pull-requests-updated` event is emitted

#### Scenario: Recovery clears the reason

- **WHEN** a refresh succeeds after an unavailable snapshot
- **THEN** the snapshot is ok and carries no reason

### Requirement: Failures Are Reported on Standard Error

`specforge-serve` and the desktop application SHALL write one line to standard error when the poller's failure reason changes, including when the first failure occurs, and one line when a refresh succeeds after a failure. A refresh that fails with the same reason as the previous one SHALL write nothing. A failed detail read SHALL write one line. The terminal frontend SHALL write none of these lines.

Each line SHALL name the provider, the HTTP method, the request's path as a template (with `{workspace}`, `{repo}`, `{user}` and `{id}` in place of the values), the reason, and, for a transport failure, the TLS or I/O error's own message. A line SHALL NOT contain the token, any header, any reply body, a query string, or a concrete workspace, repository, user or pull-request identifier. This requirement leaves the *Privacy and Safety* requirement intact.

#### Scenario: The first failure is reported once

- **WHEN** three consecutive refreshes fail because the certificate chain is not trusted
- **THEN** standard error receives one line naming `GET /2.0/user`, the reason "certificate not trusted" and the TLS error's message
- **AND** the second and third refreshes write nothing

#### Scenario: Recovery is reported

- **WHEN** a refresh succeeds after a reported failure
- **THEN** standard error receives one line saying BitBucket has recovered

#### Scenario: A line carries no identifiers or secrets

- **WHEN** the pull-request listing for workspace `acme` fails with HTTP 500
- **THEN** the line names `GET /2.0/workspaces/{workspace}/pullrequests/{user}` and status 500
- **AND** it contains neither `acme`, the username, the token, nor any reply text

#### Scenario: A failed detail read is reported

- **WHEN** a detail read's request for pull request 7 of `acme/web` fails with a timeout
- **THEN** standard error receives one line naming `GET /2.0/repositories/{workspace}/{repo}/pullrequests/{id}` and the reason "timeout"
