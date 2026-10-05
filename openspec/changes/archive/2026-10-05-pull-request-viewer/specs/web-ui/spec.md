## MODIFIED Requirements

### Requirement: Local Self-Served Web Server

The application SHALL be able to serve a browser-renderable web UI from a local HTTP server that reflects this machine's registered OpenSpec workspaces. The web UI is optional and off by default.

The **embedded** server (the desktop app's serve toggle) SHALL bind to the loopback interface (`127.0.0.1`) only and SHALL NOT expose itself on any non-loopback network interface. No persisted configuration value SHALL be capable of moving it off loopback.

The **standalone** server binary SHALL bind to the loopback interface by default, and MAY bind a non-loopback interface only when the operator explicitly requests one as a command-line argument to that invocation (or its documented environment-variable fallback). The requested bind address SHALL NOT be readable from, or persisted to, the shared application settings, so that a non-loopback bind is always an explicit act of the invocation rather than a stored state that outlives it. A bind address that cannot be parsed SHALL be a fatal startup error, never a silent fallback to the default.

When bound to a non-loopback interface the server SHALL announce, at startup, that the UI is reachable from the network and unauthenticated, so the operator is told what the invocation published. The announcement SHALL also state that the served UI discloses the content of the listed pull requests — their changed files and conversations — read with the serving host's credentials. The listed pull requests are those that GitHub or BitBucket pull-request tracking lists while it is enabled: on GitHub, the open pull requests the account authored and those awaiting its review; on BitBucket, those the account authored (see the *One Constant Read-Only Query* requirement in the `github-pull-requests` capability and the *Authored Pull-Request Discovery* requirement in the `bitbucket-pull-requests` capability). Their repositories need not be cloned on the serving host, so the warning that the UI is unauthenticated would not, on its own, tell the operator that their content is served.

#### Scenario: Web UI is disabled by default

- **WHEN** the application starts with no web-serving configuration enabled
- **THEN** no local HTTP server is listening
- **AND** the desktop and terminal frontends behave exactly as before

#### Scenario: Enabled embedded server binds loopback only

- **WHEN** the web UI is enabled on a configured port from the desktop app
- **THEN** the server accepts connections on `127.0.0.1:<port>`
- **AND** the server does not accept connections addressed to a non-loopback interface

#### Scenario: The embedded server cannot be moved off loopback

- **WHEN** the shared application settings are edited by any means available to a user
- **THEN** there is no setting whose value causes the embedded server to bind a non-loopback interface

#### Scenario: Standalone server binds loopback by default

- **WHEN** the standalone server binary is started with no bind address requested
- **THEN** it binds the loopback interface, exactly as before this capability existed
- **AND** it does not accept connections addressed to a non-loopback interface

#### Scenario: Standalone server binds a requested network interface

- **WHEN** the standalone server binary is started with a non-loopback bind address requested on the command line
- **THEN** it accepts connections addressed to that interface
- **AND** it prints, before serving, that the UI is reachable from the network and unauthenticated

#### Scenario: The network-bind announcement names the listed pull requests

- **WHEN** the standalone server binary is started with a non-loopback bind address requested on the command line
- **THEN** it prints, before serving, that the served UI also discloses the listed pull requests' content
- **AND** it states that this content is read with the serving host's credentials

#### Scenario: A malformed bind address is fatal

- **WHEN** the standalone server binary is started with a bind address that cannot be parsed
- **THEN** it exits with a non-zero status and a message naming the offending value
- **AND** it does not fall back to the default bind and does not begin serving

#### Scenario: Served UI reflects this machine's workspaces

- **WHEN** a browser loads the served UI
- **THEN** the workspaces, changes, and dashboard rendered are those of the registered workspaces on the machine running the server
- **AND** they are the same data the desktop and terminal frontends render from the shared `AppService`

### Requirement: Localhost Trust Boundary

The web server SHALL treat reachability and authorization as separate concerns: the bound interface controls *who can open a socket*, while a request-authority allowlist controls *which page may drive the API*.

While the server is bound to the loopback interface it SHALL validate every request's `Origin` and `Host` headers against an allowlist of trusted authorities and reject any request that does not match, so that an unrelated web page in the user's browser cannot drive workspace-registration or artifact-reading commands against the user's local filesystem.

The allowlist SHALL always include the loopback authorities (`localhost`, `127.0.0.1`, `::1`). It MAY additionally include the host's own Tailscale (MagicDNS) name when Tailscale Serve support is enabled (see *Tailscale Serve Access*). While the server is bound to loopback the allowlist SHALL contain only specific, known authorities — never a wildcard and never "any non-loopback host" — so that relaxing reachability through the Tailscale proxy never relaxes the cross-origin defense.

When, and only when, the operator has explicitly requested a non-loopback bind (see *Local Self-Served Web Server*), the server SHALL accept any `Host` and any `Origin`. This is a deliberate trade, not an oversight: an allowlist that rejected the address the server was just told to publish on would refuse every request the flag exists to serve. In this mode the allowlist provides no cross-origin and no DNS-rebinding defense, and the network the server is published on — together with every site any browser on that network visits — is the entire trust boundary. The mode SHALL remain unreachable except by explicit request on the invocation, and SHALL NOT be available to the embedded server at all.

Separately from the authority check, every response that serves the application shell (see *Deep-Link Durability of the Served Bundle*) SHALL carry `Content-Security-Policy: frame-ancestors 'none'` and `X-Frame-Options: DENY`, so that no other page can host the served UI in a frame, and `X-DNS-Prefetch-Control: off`, so that the browser does not resolve the hosts of the links in rendered content, such as a pull request's description and comments, before any of them is followed. Both entry points SHALL send these headers in every configuration: unlike the allowlist, they are neither set aside by an explicit network bind nor relaxed by Tailscale Serve support. The content-security policy the server sends SHALL consist of the `frame-ancestors` directive alone: it governs which pages may frame the shell, never what the shell loads, so remote images in the user's own artifacts still load.

#### Scenario: A cross-origin page is refused

- **WHEN** the server is bound to loopback and a request arrives whose `Origin` is not in the allowlist
- **THEN** the server refuses to dispatch the command
- **AND** no workspace is registered and no artifact is read as a result

#### Scenario: The application's own served UI is accepted

- **WHEN** a request originates from the UI the server itself serves (a loopback origin, or an enabled-and-trusted Tailscale origin)
- **THEN** the request passes the origin check
- **AND** the command is dispatched normally

#### Scenario: Loopback is always trusted

- **WHEN** a request arrives with a loopback `Host` and `Origin`
- **THEN** it passes the authority check regardless of whether Tailscale support is enabled

#### Scenario: A non-loopback authority that is not the configured one is refused

- **WHEN** the server is bound to loopback and a request arrives whose `Host` or `Origin` is a non-loopback authority that is not the server's own trusted Tailscale name
- **THEN** the server refuses the request
- **AND** does so even if the authority is some other `.ts.net` name

#### Scenario: An explicit network bind accepts any authority

- **WHEN** the standalone server is bound to a non-loopback interface at the operator's request, and a request arrives whose `Host` and `Origin` are arbitrary values
- **THEN** the authority check does not reject it
- **AND** the command is dispatched

#### Scenario: The default configuration is unchanged by the existence of the flag

- **WHEN** the server is started without requesting a non-loopback bind
- **THEN** the authority allowlist behaves exactly as it did before this capability existed
- **AND** a request bearing an arbitrary `Host` is refused

#### Scenario: The served shell refuses to be framed

- **WHEN** a browser requests the application shell, at the root or at a deep address
- **THEN** the response carries `Content-Security-Policy: frame-ancestors 'none'` and `X-Frame-Options: DENY`
- **AND** the browser refuses to display the served UI inside a frame on any page, whatever that page's origin

#### Scenario: The served shell turns off DNS prefetching

- **WHEN** a browser requests the application shell, at the root or at a deep address
- **THEN** the response carries `X-DNS-Prefetch-Control: off`

#### Scenario: The served policy governs framing only

- **WHEN** a browser requests the application shell
- **THEN** the response's `Content-Security-Policy` header carries the `frame-ancestors 'none'` directive and no other
- **AND** a remote image in a workspace artifact still loads when that artifact is rendered in the browser skin

#### Scenario: The shell headers hold in every configuration

- **WHEN** the application shell is served by the embedded server, by the standalone server on loopback, by the standalone server under an explicit network bind, or with Tailscale Serve support enabled
- **THEN** the response carries `Content-Security-Policy: frame-ancestors 'none'`, `X-Frame-Options: DENY` and `X-DNS-Prefetch-Control: off` in every case

### Requirement: Tailscale Serve Access

The web server SHALL support being reached over a Tailscale tailnet via `tailscale serve` without binding any non-loopback interface itself. When Tailscale Serve support is enabled, the server SHALL add the host's own Tailscale (MagicDNS) name to the request-authority allowlist (for both `Origin` and `Host`, because `tailscale serve` preserves the original `Host`), and SHALL continue to reject every other non-loopback authority. Tailscale Serve support SHALL be off by default.

The server SHALL NOT bind a non-loopback interface to provide this access; reachability beyond the machine is provided exclusively by the external `tailscale serve` proxy, which connects to the loopback port. An explicitly requested non-loopback bind (see *Local Self-Served Web Server*) is a separate, independent mechanism and is never required for, nor implied by, Tailscale Serve access. Tailscale Funnel (public-internet exposure) is explicitly not supported.

The server SHOULD determine its own Tailscale name from the local Tailscale state rather than requiring the user to enter it, and SHALL allow a manually configured name as an override and as a fallback when the name cannot be determined. When no Tailscale name is available, the server SHALL trust no non-loopback authority (fail closed).

The server MAY enforce per-user authorization for Tailscale-proxied requests: when an allow-list of Tailscale user logins is configured, a request bearing the trusted Tailscale name SHALL be accepted only if it also carries a Tailscale identity (`Tailscale-User-Login`) present in that list; when the allow-list is empty, the tailnet itself is the trust boundary. The identity SHALL be trusted only on the basis that the server binds loopback (so the header cannot be forged by a remote peer, only by an already-trusted local process). Loopback requests SHALL never require a login.

Because that basis does not survive a non-loopback bind — any peer able to reach the port could then supply the header itself — the server SHALL refuse to start when a non-loopback bind is requested while a non-empty login allow-list is configured, naming both inputs in the error. It SHALL NOT start with the login gate silently disabled, so a configured restriction can never appear to be in force while it is not. An enabled Tailscale integration with an *empty* login allow-list SHALL NOT block startup, since it only widens the authority allowlist that an explicit network bind already sets aside.

Tailscale Serve reaches the server while it stays bound to loopback, so the network-bind announcement (see *Local Self-Served Web Server*) never prints for it. The Tailscale Serve setting's description SHALL therefore make that disclosure itself: it SHALL state that, while GitHub or BitBucket pull-request tracking is enabled and no logins are listed, every device on the tailnet can read the content of the listed pull requests, read with the serving host's credentials, and it SHALL suggest a login allow-list to restrict that access.

#### Scenario: Disabled by default

- **WHEN** the server starts with Tailscale Serve support not enabled
- **THEN** no Tailscale name is in the authority allowlist
- **AND** a request bearing a tailnet `Host`/`Origin` is refused

#### Scenario: A serve-proxied request is accepted when enabled

- **WHEN** Tailscale Serve support is enabled with the host's tailnet name resolved, and a request arrives (via `tailscale serve`) whose `Host` and `Origin` are that tailnet name
- **THEN** the request passes the authority check and is dispatched

#### Scenario: Cross-origin is still refused with Tailscale enabled

- **WHEN** Tailscale Serve support is enabled and a request arrives whose `Origin` is a third-party site (e.g. a page the user opened on a tailnet device's browser)
- **THEN** the server refuses it, exactly as in the loopback-only configuration

#### Scenario: The app does not bind a non-loopback interface

- **WHEN** Tailscale Serve support is enabled
- **THEN** the server still binds only the loopback interface
- **AND** it does not accept connections addressed directly to a non-loopback interface (only `tailscale serve`, connecting from loopback, reaches it)

#### Scenario: Per-user authorization restricts to allowed logins

- **WHEN** an allow-list of Tailscale logins is configured and a serve-proxied request carries a `Tailscale-User-Login` that is in the list
- **THEN** the request is accepted
- **AND** an otherwise-identical request whose login is not in the list is refused

#### Scenario: A network bind with a configured login allow-list refuses to start

- **WHEN** the standalone server is started with a non-loopback bind requested while a non-empty Tailscale login allow-list is configured
- **THEN** it exits with a non-zero status and an error naming both the requested bind and the configured allow-list
- **AND** it does not begin serving with the login gate disabled

#### Scenario: An enabled integration without a login allow-list does not block a network bind

- **WHEN** the standalone server is started with a non-loopback bind requested while Tailscale Serve support is enabled but the login allow-list is empty
- **THEN** the server starts and serves

#### Scenario: The resolved Tailscale name is discoverable to the user

- **WHEN** Tailscale Serve support is enabled
- **THEN** the user can see which Tailscale name the server has resolved and trusted (so a wrong or stale name is diagnosable)

#### Scenario: The setting discloses the listed pull requests to the tailnet

- **WHEN** the Settings view renders the Tailscale Serve setting in the desktop app
- **THEN** its description states that, while GitHub or BitBucket pull-request tracking is enabled and no logins are listed, every device on the tailnet can read the listed pull requests' content, read with the serving host's credentials
- **AND** it suggests listing logins to restrict that access
