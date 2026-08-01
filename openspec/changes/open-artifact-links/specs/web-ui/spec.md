## MODIFIED Requirements

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

## ADDED Requirements

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
