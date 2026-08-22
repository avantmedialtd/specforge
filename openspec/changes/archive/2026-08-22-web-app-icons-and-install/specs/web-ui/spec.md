## MODIFIED Requirements

### Requirement: Deep-Link Durability of the Served Bundle

The web server SHALL answer a request for any path that is not a bundled static asset with the application shell, so that an address deep-linking into the UI can be opened directly, reloaded, and bookmarked without the server needing to understand the address grammar.

Requests that do name a bundled static asset SHALL continue to be served as that asset with its own content type, so the fallback never shadows the bundle's own files. Address resolution remains entirely a frontend concern — see the *Cold-Load Address Resolution* requirement in the `view-routing` capability — and the server SHALL NOT be required to enumerate or validate addressable paths.

The shell fallback SHALL be bounded by the bundle's own static-asset namespace: a request naming a path inside that namespace for which no asset exists SHALL receive a not-found response rather than the shell, so that a consumer asking for an image or a manifest is never handed an HTML document under that request. The namespace SHALL be defined as an explicit set — the bundle's generated asset directory together with a fixed list of well-known root-level files such as the icons and the web app manifest — and SHALL NOT be inferred from the presence of a file extension, because addresses deep-linking into the UI are built from workspace and change identifiers that may themselves contain dots. Consumers probe well-known icon paths directly regardless of what the document declares, so this boundary is required even when every declared icon exists.

When no frontend bundle is present the server SHALL continue to report that the UI must be built, rather than returning an empty shell.

#### Scenario: A deep address is served the application shell

- **WHEN** a browser requests a path that deep-links into the UI and matches no bundled static asset
- **THEN** the server responds with the application shell
- **AND** the frontend resolves the address itself

#### Scenario: Reloading a deep address works

- **WHEN** the user reloads the browser at an address naming a change artifact
- **THEN** the page loads and renders that artifact
- **AND** the user is not returned to the home surface

#### Scenario: A static asset is not shadowed by the fallback

- **WHEN** a browser requests a path that names a bundled static asset
- **THEN** the server responds with that asset and its own content type
- **AND** it does not respond with the application shell

#### Scenario: A missing asset in the static namespace is not answered with the shell

- **WHEN** a browser requests a well-known icon path that the bundle does not contain
- **THEN** the server responds with a not-found status
- **AND** it does not respond with the application shell or an HTML content type

#### Scenario: A deep address containing a dot is still served the shell

- **WHEN** a browser requests a deep address whose workspace or change identifier contains a dot
- **THEN** the server responds with the application shell
- **AND** it does not respond with a not-found status

#### Scenario: A missing bundle still reports a build hint

- **WHEN** a request arrives and no frontend bundle is present
- **THEN** the server reports that the web UI assets were not found and must be built

### Requirement: Link Handling in the Browser Skin

In the web UI, a link click inside rendered artifact markdown SHALL NOT navigate the serving page. An absolute `http` or `https` link SHALL open in a new browser tab whose window is isolated from the opener (`rel="noopener noreferrer"` semantics).

Where the bundle is running as an installed standalone application there is no sibling tab to open, and the platform presents the opener-isolated destination as an in-application browser view instead — see the *Installed App Presents Its Own Icon and Window* requirement in the `web-app-install` capability. That presentation SHALL satisfy this requirement: what is normative is that the destination opens isolated from the opener and that the serving page itself does not navigate, not the particular window furniture the platform provides.

A relative link to a workspace file SHALL NOT navigate and SHALL NOT be fetched from the server; the UI SHALL instead present the link's target path in a non-navigating way (for example a tooltip or inline affordance), because the target exists on the serving host's filesystem, not necessarily the viewer's machine.

Notwithstanding *Command Transport Mirrors the In-Process Command Surface*, the web transport SHALL NOT expose any operation that opens files or URLs on the serving host: the desktop open operation is absent from the web dispatch surface, so no browser request can cause the server machine to launch an application.

#### Scenario: An external link opens in a new tab

- **WHEN** the user clicks an `http(s)` link in a rendered artifact in the web UI
- **THEN** the URL opens in a new browser tab with an opener-isolated window
- **AND** the SpecForge page itself does not navigate

#### Scenario: An external link in an installed app opens without navigating the app

- **WHEN** the user clicks an `http(s)` link in a rendered artifact while the bundle is running as an installed standalone application
- **THEN** the URL opens isolated from the opener in the platform's in-application browser view
- **AND** the SpecForge window itself does not navigate away from the artifact

#### Scenario: A workspace file link degrades without navigating

- **WHEN** the user clicks a relative link to an `.html` mockup in the web UI
- **THEN** the page does not navigate
- **AND** the UI presents the link's target path without opening anything

#### Scenario: The web surface cannot open files on the server

- **WHEN** any request is made against the web transport's dispatch surface
- **THEN** no available operation opens a file or URL on the serving host

## ADDED Requirements

### Requirement: Event Stream Recovers After Document Suspension

The frontend SHALL restore the event stream when the document is restored from a suspended or frozen state and the stream is no longer open, and SHALL re-read current state through the ordinary command surface once it has done so.

Reconnection alone SHALL NOT be treated as recovery. The stream carries no event identifiers and the server retains no event history — a lagging receiver is deliberately skipped rather than replayed — so a client that reconnects after a suspension has no way to learn what changed while it was suspended. Without the re-read, the UI would continue to present pre-suspension state until some unrelated event happened to arrive, while appearing live.

The frontend SHALL NOT replace a stream that is still open, so that ordinary visibility changes do not churn connections or re-read state without cause. The re-read SHALL be single-flight, so overlapping restorations collapse into one.

This requirement SHALL NOT be satisfied by adding replay to the server: the event broadcast is deliberately lossy, and retaining history to serve one consumer's lifecycle would reverse that decision in the shared application layer.

#### Scenario: A suspended app resumes with live state

- **WHEN** the document is suspended by the operating system and later restored, and the event stream is no longer open
- **THEN** the frontend establishes a new event stream
- **AND** it re-reads current state through the command surface
- **AND** the UI reflects changes that occurred while the document was suspended

#### Scenario: A healthy stream is not replaced

- **WHEN** the document is hidden and shown again while its event stream remains open
- **THEN** the frontend does not replace the stream
- **AND** it does not re-read state

#### Scenario: Overlapping restorations read state once

- **WHEN** the document is restored more than once before an in-flight state re-read completes
- **THEN** the frontend performs a single re-read rather than one per restoration

#### Scenario: The server gains no event history

- **WHEN** the event stream implementation is inspected
- **THEN** it assigns no event identifiers and retains no buffer of past events
- **AND** a lagging receiver is skipped rather than replayed
