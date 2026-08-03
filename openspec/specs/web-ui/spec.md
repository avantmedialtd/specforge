# web-ui Specification

## Purpose

Defines an optional, local, self-served browser skin for SpecForge that renders the same OpenSpec state the desktop and terminal frontends do — a transport adapter over the shared headless application service rather than new behaviour. It covers the loopback HTTP command endpoint (mirroring the in-process `invoke(command, args)` surface), the one-way SSE event stream (reproducing the desktop's named cache events), the single host-detected frontend bundle, the two entry points (an embedded toggle in the desktop app and a standalone headless server), the localhost trust boundary, and the web-flavoured affordances (path-based workspace registration, hidden desktop-only settings). Parallels the `terminal-ui` capability: a thin presentation transport with no parsing, watching, git, or dashboard computation of its own.

## Requirements

### Requirement: Local Self-Served Web Server

The application SHALL be able to serve a browser-renderable web UI from a local HTTP server that reflects this machine's registered OpenSpec workspaces. The web UI is optional and off by default.

The **embedded** server (the desktop app's serve toggle) SHALL bind to the loopback interface (`127.0.0.1`) only and SHALL NOT expose itself on any non-loopback network interface. No persisted configuration value SHALL be capable of moving it off loopback.

The **standalone** server binary SHALL bind to the loopback interface by default, and MAY bind a non-loopback interface only when the operator explicitly requests one as a command-line argument to that invocation (or its documented environment-variable fallback). The requested bind address SHALL NOT be readable from, or persisted to, the shared application settings, so that a non-loopback bind is always an explicit act of the invocation rather than a stored state that outlives it. A bind address that cannot be parsed SHALL be a fatal startup error, never a silent fallback to the default.

When bound to a non-loopback interface the server SHALL announce, at startup, that the UI is reachable from the network and unauthenticated, so the operator is told what the invocation published.

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

#### Scenario: A malformed bind address is fatal

- **WHEN** the standalone server binary is started with a bind address that cannot be parsed
- **THEN** it exits with a non-zero status and a message naming the offending value
- **AND** it does not fall back to the default bind and does not begin serving

#### Scenario: Served UI reflects this machine's workspaces

- **WHEN** a browser loads the served UI
- **THEN** the workspaces, changes, and dashboard rendered are those of the registered workspaces on the machine running the server
- **AND** they are the same data the desktop and terminal frontends render from the shared `AppService`

### Requirement: Command Transport Mirrors the In-Process Command Surface

The web server SHALL expose a single command endpoint accepting a command name and an arguments object, dispatching to the same `AppService` / settings / watcher operations the desktop frontend invokes, and returning the operation's result or a structured error. The wire shape SHALL mirror the existing `invoke(command, args)` contract so the frontend's command layer differs only in transport.

The mirror contract covers read and configuration operations. Operations whose effect acts on the serving host's own machine — such as the artifact-link open operation (see *Link Handling in the Browser Skin*) — SHALL be deliberately absent from the dispatch surface rather than mirrored, and requests naming them SHALL be rejected as unsupported.

#### Scenario: A command is dispatched and its result returned

- **WHEN** the browser sends a command request naming a supported command with its arguments
- **THEN** the server invokes the corresponding shared operation
- **AND** returns its result serialized identically to the desktop frontend's result for that command

#### Scenario: A failing command returns a structured error

- **WHEN** a dispatched command returns an error
- **THEN** the server responds with an error envelope the frontend can surface
- **AND** does not crash the server or the stream for other requests

#### Scenario: An unknown command is rejected

- **WHEN** the browser sends a request naming a command the dispatch table does not know
- **THEN** the server responds with an error indicating the command is unsupported
- **AND** no operation is performed

#### Scenario: A host-side effectful command is not mirrored

- **WHEN** the browser sends a command request naming the artifact-link open operation
- **THEN** the server rejects it as unsupported
- **AND** no file or URL is opened on the serving host

### Requirement: Event Transport Reproduces the Frontend Event Contract

The web server SHALL stream cache-change events to the browser as a one-way server-to-client stream (Server-Sent Events), bridging the watcher's existing `CacheEvent` broadcast. Each streamed event's name and payload SHALL be identical to the corresponding named event the desktop frontend already listens for, so frontend event handlers require no change beyond their subscription transport.

#### Scenario: A filesystem change reaches the browser as the same event

- **WHEN** a registered workspace's OpenSpec files change while a browser is connected to the event stream
- **THEN** the browser receives an event whose name and payload match the desktop frontend's named event for that change
- **AND** the frontend's existing handler for that event runs unmodified

#### Scenario: Event name-and-payload mapping has a single source

- **WHEN** the mapping from a `CacheEvent` variant to its event name and payload is needed by both the desktop forwarder and the web stream
- **THEN** both obtain it from one shared mapping in the application layer
- **AND** the two transports emit identical wire shapes for the same `CacheEvent`

#### Scenario: The stream reconnects without losing the session

- **WHEN** the event stream connection drops transiently
- **THEN** the browser re-establishes the stream automatically
- **AND** subsequent events continue to be delivered

### Requirement: Single Frontend Bundle, Host-Detected Transport

The browser SHALL render from the same frontend bundle the desktop app uses, selecting its backend transport at runtime by detecting whether it is hosted inside the native shell. When hosted natively it SHALL use in-process invocation and native event subscription; when served over HTTP it SHALL use the HTTP command endpoint and the event stream.

#### Scenario: Same bundle runs natively

- **WHEN** the bundle runs inside the native desktop shell
- **THEN** it uses the in-process command and event mechanisms
- **AND** native-only affordances (window controls, native folder dialog) are active

#### Scenario: Same bundle runs in a browser

- **WHEN** the bundle runs in a browser served by the local web server
- **THEN** it uses the HTTP command endpoint and the event stream
- **AND** native-only affordances are replaced by their web equivalents or hidden

### Requirement: Two Entry Points Over One Server Core

The web server SHALL be implemented as a library that accepts an existing `AppService` and serves it, with two entry points: an embedded mode in which the running desktop app serves the web UI from the same `AppService` it already holds, and a standalone mode in which a headless binary bootstraps its own `AppService` from the shared configuration directory and serves only the web UI.

#### Scenario: Embedded mode shares the desktop's live state

- **WHEN** the desktop app enables web serving
- **THEN** the web server is driven by the same `AppService` instance as the desktop UI
- **AND** the browser observes the same live state and the same single watcher, with no second writer of application state

#### Scenario: Standalone mode serves without a GUI

- **WHEN** the standalone server binary is started against the shared configuration directory
- **THEN** it bootstraps its own `AppService`, populates it, and serves the web UI
- **AND** requires no tray, dock, or native window

### Requirement: Localhost Trust Boundary

The web server SHALL treat reachability and authorization as separate concerns: the bound interface controls *who can open a socket*, while a request-authority allowlist controls *which page may drive the API*.

While the server is bound to the loopback interface it SHALL validate every request's `Origin` and `Host` headers against an allowlist of trusted authorities and reject any request that does not match, so that an unrelated web page in the user's browser cannot drive workspace-registration or artifact-reading commands against the user's local filesystem.

The allowlist SHALL always include the loopback authorities (`localhost`, `127.0.0.1`, `::1`). It MAY additionally include the host's own Tailscale (MagicDNS) name when Tailscale Serve support is enabled (see *Tailscale Serve Access*). While the server is bound to loopback the allowlist SHALL contain only specific, known authorities — never a wildcard and never "any non-loopback host" — so that relaxing reachability through the Tailscale proxy never relaxes the cross-origin defense.

When, and only when, the operator has explicitly requested a non-loopback bind (see *Local Self-Served Web Server*), the server SHALL accept any `Host` and any `Origin`. This is a deliberate trade, not an oversight: an allowlist that rejected the address the server was just told to publish on would refuse every request the flag exists to serve. In this mode the allowlist provides no cross-origin and no DNS-rebinding defense, and the network the server is published on — together with every site any browser on that network visits — is the entire trust boundary. The mode SHALL remain unreachable except by explicit request on the invocation, and SHALL NOT be available to the embedded server at all.

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

### Requirement: Tailscale Serve Access

The web server SHALL support being reached over a Tailscale tailnet via `tailscale serve` without binding any non-loopback interface itself. When Tailscale Serve support is enabled, the server SHALL add the host's own Tailscale (MagicDNS) name to the request-authority allowlist (for both `Origin` and `Host`, because `tailscale serve` preserves the original `Host`), and SHALL continue to reject every other non-loopback authority. Tailscale Serve support SHALL be off by default.

The server SHALL NOT bind a non-loopback interface to provide this access; reachability beyond the machine is provided exclusively by the external `tailscale serve` proxy, which connects to the loopback port. An explicitly requested non-loopback bind (see *Local Self-Served Web Server*) is a separate, independent mechanism and is never required for, nor implied by, Tailscale Serve access. Tailscale Funnel (public-internet exposure) is explicitly not supported.

The server SHOULD determine its own Tailscale name from the local Tailscale state rather than requiring the user to enter it, and SHALL allow a manually configured name as an override and as a fallback when the name cannot be determined. When no Tailscale name is available, the server SHALL trust no non-loopback authority (fail closed).

The server MAY enforce per-user authorization for Tailscale-proxied requests: when an allow-list of Tailscale user logins is configured, a request bearing the trusted Tailscale name SHALL be accepted only if it also carries a Tailscale identity (`Tailscale-User-Login`) present in that list; when the allow-list is empty, the tailnet itself is the trust boundary. The identity SHALL be trusted only on the basis that the server binds loopback (so the header cannot be forged by a remote peer, only by an already-trusted local process). Loopback requests SHALL never require a login.

Because that basis does not survive a non-loopback bind — any peer able to reach the port could then supply the header itself — the server SHALL refuse to start when a non-loopback bind is requested while a non-empty login allow-list is configured, naming both inputs in the error. It SHALL NOT start with the login gate silently disabled, so a configured restriction can never appear to be in force while it is not. An enabled Tailscale integration with an *empty* login allow-list SHALL NOT block startup, since it only widens the authority allowlist that an explicit network bind already sets aside.

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

### Requirement: Web-Flavoured Workspace Registration

In the web UI the user SHALL be able to register a workspace by supplying its path without a native OS folder dialog, and the supplied path SHALL flow into the same registration operation the desktop frontend uses.

#### Scenario: Registering a workspace by path in the browser

- **WHEN** the user supplies a workspace path in the web UI and confirms registration
- **THEN** the path is passed to the shared workspace-registration operation
- **AND** registration succeeds or fails by the same rules as the desktop frontend (including the `openspec/` subdirectory requirement)

### Requirement: Desktop-Only Settings Are Hidden in the Web UI

Settings that have no meaning for a browser skin (launch-on-login, OS-level notifications, tray behaviour) SHALL be hidden when the UI is served over HTTP, reusing the existing convention of omitting a control when its backing query reports the control is not applicable.

#### Scenario: Launch-on-login is absent in the browser

- **WHEN** the Settings view renders in the web UI
- **THEN** the launch-on-login control is not shown
- **AND** the remaining, applicable settings render and function normally

### Requirement: Read-Only Parity With the Desktop Frontend

The web UI SHALL provide read-only parity with the desktop frontend and SHALL NOT introduce write operations against OpenSpec artifacts (such as toggling tasks or editing files) that the desktop frontend does not itself provide.

#### Scenario: No artifact mutation from the browser

- **WHEN** the user interacts with the web UI
- **THEN** the available actions are the same read and configuration actions the desktop frontend exposes
- **AND** OpenSpec artifact files are not modified through the web UI

### Requirement: Link Handling in the Browser Skin

In the web UI, a link click inside rendered artifact markdown SHALL NOT navigate the serving page. An absolute `http` or `https` link SHALL open in a new browser tab whose window is isolated from the opener (`rel="noopener noreferrer"` semantics).

A relative link to a workspace file SHALL NOT navigate and SHALL NOT be fetched from the server; the UI SHALL instead present the link's target path in a non-navigating way (for example a tooltip or inline affordance), because the target exists on the serving host's filesystem, not necessarily the viewer's machine.

Notwithstanding *Command Transport Mirrors the In-Process Command Surface*, the web transport SHALL NOT expose any operation that opens files or URLs on the serving host: the desktop open operation is absent from the web dispatch surface, so no browser request can cause the server machine to launch an application.

#### Scenario: An external link opens in a new tab

- **WHEN** the user clicks an `http(s)` link in a rendered artifact in the web UI
- **THEN** the URL opens in a new browser tab with an opener-isolated window
- **AND** the SpecForge page itself does not navigate

#### Scenario: A workspace file link degrades without navigating

- **WHEN** the user clicks a relative link to an `.html` mockup in the web UI
- **THEN** the page does not navigate
- **AND** the UI presents the link's target path without opening anything

#### Scenario: The web surface cannot open files on the server

- **WHEN** any request is made against the web transport's dispatch surface
- **THEN** no available operation opens a file or URL on the serving host
