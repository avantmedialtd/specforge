## ADDED Requirements

### Requirement: GitHub Connection Reuse

The system SHALL reuse an open connection to `api.github.com`, where one is idle, for the requests of a detail read, a file read or an image read, rather than opening a new TCP and TLS connection for each request. These requests are a detail read's GraphQL `POST` and its files pages, and a file or image read's compare and contents `GET`s.

A connection that has been idle for 15 seconds or more SHALL NOT be reused; the request SHALL open a new one instead.

$$\text{reused}(c) \implies \text{idle}(c) < 15\,\text{s}$$

Reuse SHALL NOT relax any part of the *GitHub Privacy and Safety* requirement:

- Every request SHALL carry the token in its own `Authorization` header, built from the token current when that request is built. A connection SHALL carry no credential, header or cookie from an earlier request to a later one.
- No request SHALL be sent through an ambient proxy configuration, whether it opens a new connection or reuses one.
- No request SHALL follow a redirect, whether it opens a new connection or reuses one.
- Each read SHALL still re-check the enabled flag before each of its requests. An idle pooled connection is not a request, and nothing SHALL be sent on it while the feature is disabled.

#### Scenario: A detail read's requests share one connection

- **WHEN** a detail read runs for a pull request with 120 changed files, so its query and its three files pages are sent within a few seconds of each other
- **THEN** all four requests travel over one connection to `api.github.com`
- **AND** each request carries the token in its own `Authorization` header

#### Scenario: A reused connection follows no redirect

- **WHEN** GitHub answers a files request sent on a reused connection with a redirect
- **THEN** the redirect is not followed and the token is sent nowhere else
- **AND** the read reports that pull request as unavailable

#### Scenario: An exported proxy is ignored

- **WHEN** `HTTPS_PROXY` or `ALL_PROXY` is set in SpecForge's environment and the poller or a detail read sends a request
- **THEN** the request goes directly to `api.github.com` and the proxy receives nothing

#### Scenario: A replaced token is sent on a reused connection

- **WHEN** the token is replaced in Settings while a connection to `api.github.com` from an earlier request is still idle in the pool, and the next request reuses it
- **THEN** that request carries the new token in its `Authorization` header and nothing of the old one

#### Scenario: A connection idle past the limit is not reused

- **WHEN** the poller sends its query 60 seconds after the previous GitHub request
- **THEN** the query opens a new connection rather than reusing the idle one
