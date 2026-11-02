## MODIFIED Requirements

### Requirement: Command Transport Mirrors the In-Process Command Surface

The web server SHALL expose a single command endpoint accepting a command name and an arguments object, dispatching supported requests to the same `AppService` / settings / watcher operations the desktop frontend invokes, and returning the operation's result or a structured error. The wire shape SHALL mirror the corresponding `invoke(command, args)` contract so the frontend's shared command layer differs only in transport where an operation is deliberately supported.

The mirror contract SHALL cover reads, local review-progress operations and non-secret configuration operations. Operations whose effect acts on the serving host's own machine — such as the artifact-link open operation (see *Link Handling in the Browser Skin*) — SHALL be deliberately absent from the dispatch surface rather than mirrored. Provider credential setters and clearers, every pull-request action-state read and draft creation/edit/recovery/start-over/confirmation/submission/resume/abandon operation, and every other provider mutation SHALL likewise be absent. Requests naming any absent operation SHALL be rejected as unsupported before action-store initialisation or any application-service mutation runs.

The shared frontend's wrappers for provider credential and review-action commands SHALL check that the host is Tauri and SHALL refuse locally before transport selection when it is not. This frontend guard is defence in depth; absence from the Rust dispatch table SHALL remain the server boundary.

#### Scenario: A command is dispatched and its result returned

- **WHEN** the browser sends a command request naming a supported read, local-progress or non-secret configuration command with its arguments
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

#### Scenario: A provider credential setter is not mirrored

- **WHEN** the browser sends a command request naming the GitHub token setter or BitBucket credential setter
- **THEN** the server rejects it as unsupported
- **AND** neither application settings nor the operating-system credential store changes

#### Scenario: A review action is not mirrored

- **WHEN** the browser sends a command request naming action-state read, draft creation, editing, recovery or start-over, submission, resume or abandonment
- **THEN** the server rejects it as unsupported
- **AND** the action store is not opened or normalised and no provider request is created

#### Scenario: Native-only wrappers fail before HTTP

- **WHEN** browser code calls a credential or review-action API wrapper directly
- **THEN** the wrapper reports that the operation requires the desktop app
- **AND** it sends no HTTP command request

### Requirement: Event Transport Reproduces the Frontend Event Contract

The web server SHALL stream the shared cache-change and service-notice events that belong to the browser frontend contract as a one-way server-to-client stream (Server-Sent Events). Each streamed event's name and payload SHALL be identical to the corresponding named event the desktop frontend listens for, so shared event handlers require no change beyond their subscription transport.

The review-action broadcast defined by the *Review Action State Synchronises Only Native Windows* requirement in the `pull-request-actions` capability SHALL be outside this shared event contract. The web server SHALL not subscribe to it, map it to an event name, retain it or stream it. Draft existence, revision, prose and operation phase SHALL therefore remain desktop-only.

#### Scenario: A filesystem change reaches the browser as the same event

- **WHEN** a registered workspace's OpenSpec files change while a browser is connected to the event stream
- **THEN** the browser receives an event whose name and payload match the desktop frontend's named event for that change
- **AND** the frontend's existing handler for that event runs unmodified

#### Scenario: Event name-and-payload mapping has a single source

- **WHEN** the mapping from a shared cache event or service notice to its event name and payload is needed by both the desktop forwarder and the web stream
- **THEN** both obtain it from one shared mapping in the application layer
- **AND** the two transports emit identical wire shapes for the same shared event

#### Scenario: Review-action events are not shared events

- **WHEN** a native review draft or operation changes while browser clients are connected
- **THEN** the SSE stream emits no event for that change
- **AND** the web server reads no action state for it

#### Scenario: The stream reconnects without losing the session

- **WHEN** the event stream connection drops transiently
- **THEN** the browser re-establishes the stream automatically
- **AND** subsequent shared events continue to be delivered

### Requirement: Desktop-Only Settings Are Hidden in the Web UI

Settings that have no meaning for a browser skin or carry native security authority SHALL be hidden when the UI is served over HTTP. These are launch-on-login, OS-level notifications, the embedded web server and its Tailscale options, tray behaviour, and every GitHub or BitBucket credential source state, field, save action, clear action and legacy-migration control. Hiding them SHALL reuse the existing convention of omitting a control when its backing operation is not applicable.

Provider enabled switches, panel positions, review skip patterns and other non-secret integration settings SHALL remain available in their existing browser groups. When credential guidance is needed, a provider card SHALL state that credentials are managed in the desktop app, but SHALL NOT reveal a credential value or offer a mutation. A settings group that this rule leaves with no applicable setting SHALL itself be omitted from the settings view's group navigation (see the *Groups With Nothing to Offer Are Omitted* requirement in the `settings-view` capability), so the browser skin never offers an empty group.

#### Scenario: Launch-on-login is absent in the browser

- **WHEN** the Settings view renders in the web UI
- **THEN** the launch-on-login control is not shown
- **AND** the remaining applicable settings render and function normally

#### Scenario: Provider credential controls are absent in the browser

- **WHEN** the Integrations group renders in the web UI
- **THEN** it shows no GitHub token or BitBucket username/token field, active/stored source state, save action, clear action or legacy re-save control
- **AND** provider enabled switches and applicable non-secret settings remain

#### Scenario: The Desktop app group is absent in the browser

- **WHEN** the Settings view renders in the web UI
- **THEN** its group navigation offers no Desktop app group
- **AND** the launch-on-login, notifications and web-server settings appear in no other group

### Requirement: Read-Only Parity With the Desktop Frontend

The web UI SHALL preserve read parity with the desktop frontend for OpenSpec state and provider data, together with the existing local configuration and local review-progress operations explicitly mirrored by *Command Transport Mirrors the In-Process Command Surface*. It SHALL NOT introduce write operations against OpenSpec artifacts or local git state, and SHALL NOT expose the desktop's provider-review capability. Reading a pull request in the browser SHALL never post, edit or delete a provider comment, reply, review or decision.

The absence of provider review writes SHALL hold on loopback, a non-loopback bind and Tailscale Serve alike. Network placement or trusted `Origin`/`Host` validation SHALL not enable them.

#### Scenario: No artifact mutation from the browser

- **WHEN** the user interacts with the web UI
- **THEN** OpenSpec artifact files and local git state are not modified through the web UI
- **AND** local review-progress and supported non-secret configuration operations remain available

#### Scenario: Pull-request reading cannot post a review

- **WHEN** the browser skin shows a pull request and its changed files
- **THEN** it offers no control or command that comments, replies, approves or requests changes
- **AND** opening, refreshing or marking review progress sends no provider mutation

#### Scenario: Publishing the server does not widen review authority

- **WHEN** the server is explicitly bound off loopback or published through Tailscale Serve
- **THEN** connected browsers receive the same provider-read-only command surface as loopback clients
- **AND** no provider credential or review mutation command becomes available
