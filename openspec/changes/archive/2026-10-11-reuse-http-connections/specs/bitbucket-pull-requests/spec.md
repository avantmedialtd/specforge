## ADDED Requirements

### Requirement: Connection Reuse

The system SHALL reuse an open connection to `api.bitbucket.org`, where one is idle, rather than opening a new TCP and TLS connection for each request. This applies to the requests of one poll cycle, where the poller's account, workspace and pull-request listings follow one another, and to the requests of a detail read or an image read.

A connection that has been idle for 15 seconds or more SHALL NOT be reused; the request SHALL open a new one instead.

$$\text{reused}(c) \implies \text{idle}(c) < 15\,\text{s}$$

Reuse SHALL NOT relax any part of the *Privacy and Safety* requirement:

- Every request SHALL carry the credential in its own `Authorization` header, built from the credentials current when that request is built. A connection SHALL carry no credential, header or cookie from an earlier request to a later one.
- No request SHALL be sent through an ambient proxy configuration, whether it opens a new connection or reuses one.
- A detail read's and an image read's requests SHALL follow no redirect, whether they open a new connection or reuse one.
- Each read SHALL still re-check the enabled flag before each of its requests. An idle pooled connection is not a request, and nothing SHALL be sent on it while the feature is disabled.

#### Scenario: A poll cycle's requests share one connection

- **WHEN** the poller refreshes an account with two workspaces, sending its user, workspace-listing and two pull-request listing requests one after another
- **THEN** all four requests travel over one connection to `api.bitbucket.org`
- **AND** each request carries the credential in its own `Authorization` header

#### Scenario: A detail read's requests share one connection

- **WHEN** a detail read sends its pull-request, comments, statuses, diffstat and diff requests one after another
- **THEN** they travel over one connection to `api.bitbucket.org`
- **AND** each request carries the credential in its own `Authorization` header

#### Scenario: A reused connection follows no redirect for a detail read

- **WHEN** BitBucket answers a detail read's request sent on a reused connection with a redirect
- **THEN** the redirect is not followed and no other destination receives the credential
- **AND** the read reports that pull request as unavailable

#### Scenario: An exported proxy is ignored

- **WHEN** `HTTPS_PROXY` or `ALL_PROXY` is set in SpecForge's environment and the poller, a detail read or an image read sends a request
- **THEN** the request goes directly to `api.bitbucket.org` and the proxy receives nothing

#### Scenario: A connection idle past the limit is not reused

- **WHEN** the poller starts its next cycle 60 seconds after the previous cycle's last request
- **THEN** the cycle's first request opens a new connection rather than reusing the idle one

#### Scenario: Replaced credentials are sent on a reused connection

- **WHEN** the credentials are replaced in Settings while a connection to `api.bitbucket.org` from an earlier request is still idle in the pool, and the next request reuses it
- **THEN** that request carries the new credentials in its `Authorization` header and nothing of the old ones
