## MODIFIED Requirements

### Requirement: Installability Adds No Service Worker

The bundle SHALL NOT register a service worker, and SHALL NOT precache or serve its own assets from a client-side cache that the bundle itself manages, such as the Cache Storage API.

The entire content of the UI comes from a live local server; an offline shell would present state that cannot be refreshed while appearing operational, which is a worse failure than being unavailable. The accepted cost is that installers which require a service worker before offering an install prompt will not offer one; home-screen installation on iOS does not require one and is unaffected.

The browser's own HTTP cache, governed by the caching headers the server sends, is not such a cache. Under the *HTTP Caching of the Served Bundle* requirement in the `web-ui` capability, the application shell is always revalidated with the server before reuse, and only content-hashed assets are reused without asking. Those assets are reachable only from a shell that the live server has just confirmed. So the HTTP cache can shorten a load the server answers, but it can never stand in for the server when the server cannot answer.

#### Scenario: No service worker is registered

- **WHEN** the served bundle is loaded and has finished initializing
- **THEN** no service worker is registered for its scope
- **AND** no cache that the bundle manages serves the bundle's assets

#### Scenario: A suspended install does not present cached state as live

- **WHEN** an installed application is resumed and the server is unreachable
- **THEN** the application does not render a cached shell as though it were current
- **AND** it surfaces that state could not be read — see the *Event Stream Recovers After Document Suspension* requirement in the `web-ui` capability

#### Scenario: The HTTP cache never serves the shell without the server

- **WHEN** the shell has previously been loaded and is held in the browser's HTTP cache, and the browser loads the UI again
- **THEN** the browser revalidates the shell with the server before using it
- **AND** a content-hashed asset is reused from the HTTP cache only when the revalidated shell references it
