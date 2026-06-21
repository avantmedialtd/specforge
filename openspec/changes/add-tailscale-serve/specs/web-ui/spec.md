# web-ui

## MODIFIED Requirements

### Requirement: Localhost Trust Boundary

The web server SHALL treat reachability and authorization as separate concerns:
binding the loopback interface controls *who can open a socket*, while a
request-authority allowlist controls *which page may drive the API*. The server
SHALL validate every request's `Origin` and `Host` headers against an allowlist
of trusted authorities and reject any request that does not match, so that an
unrelated web page in the user's browser cannot drive workspace-registration or
artifact-reading commands against the user's local filesystem.

The allowlist SHALL always include the loopback authorities (`localhost`,
`127.0.0.1`, `::1`). It MAY additionally include the host's own Tailscale
(MagicDNS) name when Tailscale Serve support is enabled (see *Tailscale Serve
Access*). The allowlist SHALL contain only specific, known authorities — never a
wildcard and never "any non-loopback host" — so that relaxing reachability never
relaxes the cross-origin defense.

#### Scenario: A cross-origin page is refused

- **WHEN** a request arrives whose `Origin` is not in the allowlist
- **THEN** the server refuses to dispatch the command
- **AND** no workspace is registered and no artifact is read as a result

#### Scenario: The application's own served UI is accepted

- **WHEN** a request originates from the UI the server itself serves (a loopback
  origin, or an enabled-and-trusted Tailscale origin)
- **THEN** the request passes the origin check
- **AND** the command is dispatched normally

#### Scenario: Loopback is always trusted

- **WHEN** a request arrives with a loopback `Host` and `Origin`
- **THEN** it passes the authority check regardless of whether Tailscale support
  is enabled

#### Scenario: A non-loopback authority that is not the configured one is refused

- **WHEN** a request arrives whose `Host` or `Origin` is a non-loopback authority
  that is not the server's own trusted Tailscale name
- **THEN** the server refuses the request
- **AND** does so even if the authority is some other `.ts.net` name

## ADDED Requirements

### Requirement: Tailscale Serve Access

The web server SHALL support being reached over a Tailscale tailnet via
`tailscale serve` without binding any non-loopback interface itself. When
Tailscale Serve support is enabled, the server SHALL add the host's own Tailscale
(MagicDNS) name to the request-authority allowlist (for both `Origin` and `Host`,
because `tailscale serve` preserves the original `Host`), and SHALL continue to
reject every other non-loopback authority. Tailscale Serve support SHALL be off
by default.

The server SHALL NOT bind a non-loopback interface to provide this access;
reachability beyond the machine is provided exclusively by the external
`tailscale serve` proxy, which connects to the loopback port. Tailscale Funnel
(public-internet exposure) is explicitly not supported.

The server SHOULD determine its own Tailscale name from the local Tailscale state
rather than requiring the user to enter it, and SHALL allow a manually configured
name as an override and as a fallback when the name cannot be determined. When no
Tailscale name is available, the server SHALL trust no non-loopback authority
(fail closed).

The server MAY enforce per-user authorization for Tailscale-proxied requests:
when an allow-list of Tailscale user logins is configured, a request bearing the
trusted Tailscale name SHALL be accepted only if it also carries a Tailscale
identity (`Tailscale-User-Login`) present in that list; when the allow-list is
empty, the tailnet itself is the trust boundary. The identity SHALL be trusted
only on the basis that the server binds loopback (so the header cannot be forged
by a remote peer, only by an already-trusted local process). Loopback requests
SHALL never require a login.

#### Scenario: Disabled by default

- **WHEN** the server starts with Tailscale Serve support not enabled
- **THEN** no Tailscale name is in the authority allowlist
- **AND** a request bearing a tailnet `Host`/`Origin` is refused

#### Scenario: A serve-proxied request is accepted when enabled

- **WHEN** Tailscale Serve support is enabled with the host's tailnet name
  resolved, and a request arrives (via `tailscale serve`) whose `Host` and
  `Origin` are that tailnet name
- **THEN** the request passes the authority check and is dispatched

#### Scenario: Cross-origin is still refused with Tailscale enabled

- **WHEN** Tailscale Serve support is enabled and a request arrives whose
  `Origin` is a third-party site (e.g. a page the user opened on a tailnet
  device's browser)
- **THEN** the server refuses it, exactly as in the loopback-only configuration

#### Scenario: The app does not bind a non-loopback interface

- **WHEN** Tailscale Serve support is enabled
- **THEN** the server still binds only the loopback interface
- **AND** it does not accept connections addressed directly to a non-loopback
  interface (only `tailscale serve`, connecting from loopback, reaches it)

#### Scenario: Per-user authorization restricts to allowed logins

- **WHEN** an allow-list of Tailscale logins is configured and a serve-proxied
  request carries a `Tailscale-User-Login` that is in the list
- **THEN** the request is accepted
- **AND** an otherwise-identical request whose login is not in the list is refused

#### Scenario: The resolved Tailscale name is discoverable to the user

- **WHEN** Tailscale Serve support is enabled
- **THEN** the user can see which Tailscale name the server has resolved and
  trusted (so a wrong or stale name is diagnosable)
