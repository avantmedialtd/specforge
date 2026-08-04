## ADDED Requirements

### Requirement: Deep-Link Durability of the Served Bundle

The web server SHALL answer a request for any path that is not a bundled static asset with the application shell, so that an address deep-linking into the UI can be opened directly, reloaded, and bookmarked without the server needing to understand the address grammar.

Requests that do name a bundled static asset SHALL continue to be served as that asset with its own content type, so the fallback never shadows the bundle's own files. Address resolution remains entirely a frontend concern — see the *Cold-Load Address Resolution* requirement in the `view-routing` capability — and the server SHALL NOT be required to enumerate or validate addressable paths.

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

#### Scenario: A missing bundle still reports a build hint

- **WHEN** a request arrives and no frontend bundle is present
- **THEN** the server reports that the web UI assets were not found and must be built
