# web-ui Specification

## Purpose

Defines an optional, local, self-served browser skin for SpecForge that renders the same OpenSpec state the desktop and terminal frontends do — a transport adapter over the shared headless application service rather than new behaviour. It covers the loopback HTTP command endpoint (mirroring the in-process `invoke(command, args)` surface), the one-way SSE event stream (reproducing the desktop's named cache events), the single host-detected frontend bundle, the two entry points (an embedded toggle in the desktop app and a standalone headless server), the localhost trust boundary, and the web-flavoured affordances (path-based workspace registration, hidden desktop-only settings). Parallels the `terminal-ui` capability: a thin presentation transport with no parsing, watching, git, or dashboard computation of its own.

## Requirements

### Requirement: Local Self-Served Web Server

The application SHALL be able to serve a browser-renderable web UI from a local HTTP server that reflects this machine's registered OpenSpec workspaces. The server SHALL bind to the loopback interface (`127.0.0.1`) only and SHALL NOT expose itself on any non-loopback network interface. The web UI is optional and off by default.

#### Scenario: Web UI is disabled by default

- **WHEN** the application starts with no web-serving configuration enabled
- **THEN** no local HTTP server is listening
- **AND** the desktop and terminal frontends behave exactly as before

#### Scenario: Enabled server binds loopback only

- **WHEN** the web UI is enabled on a configured port
- **THEN** the server accepts connections on `127.0.0.1:<port>`
- **AND** the server does not accept connections addressed to a non-loopback interface

#### Scenario: Served UI reflects this machine's workspaces

- **WHEN** a browser loads the served UI
- **THEN** the workspaces, changes, and dashboard rendered are those of the registered workspaces on the machine running the server
- **AND** they are the same data the desktop and terminal frontends render from the shared `AppService`

### Requirement: Command Transport Mirrors the In-Process Command Surface

The web server SHALL expose a single command endpoint accepting a command name and an arguments object, dispatching to the same `AppService` / settings / watcher operations the desktop frontend invokes, and returning the operation's result or a structured error. The wire shape SHALL mirror the existing `invoke(command, args)` contract so the frontend's command layer differs only in transport.

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

The web server SHALL treat localhost as a shared trust boundary rather than zero-trust: it SHALL validate request origin (via the `Origin`/`Host` header against an allowlist of its own origin, optionally combined with a token the application embeds in the URL it opens) so that an unrelated web page in the user's browser cannot drive workspace-registration or artifact-reading commands against the user's local filesystem.

#### Scenario: A cross-origin page is refused

- **WHEN** a request arrives whose origin is not the server's own allowed origin
- **THEN** the server refuses to dispatch the command
- **AND** no workspace is registered and no artifact is read as a result

#### Scenario: The application's own served UI is accepted

- **WHEN** a request originates from the UI the server itself serves
- **THEN** the request passes the origin check
- **AND** the command is dispatched normally

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
