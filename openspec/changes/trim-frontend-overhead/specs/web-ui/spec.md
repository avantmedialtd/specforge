## ADDED Requirements

### Requirement: HTTP Caching of the Served Bundle

The web server SHALL tell the browser how long each part of the bundle may be reused. A reload then fetches only what changed since the last build, and no state is ever presented from a cache as though it were live.

Files in the bundle's generated asset directory (`assets/`) are named by a content hash, so a given path never changes content. The server SHALL serve them with a long-lived, immutable caching policy of one year.

Every response that serves the application shell, whether at the root, at `/index.html` or as the fallback for a deep address (see *Deep-Link Durability of the Served Bundle*), SHALL require the browser to revalidate before reuse. Each such response SHALL carry an entity tag derived from the shell's content. The same applies to the well-known root-level files whose paths stay stable across builds: the icons, the favicon and the web app manifest (see the *Served Document Declares an Icon Set* requirement in the `web-app-install` capability). They SHALL never receive the immutable policy.

A conditional request whose entity tag matches the current content SHALL be answered with a not-modified status and no body. When that response is for the application shell, it SHALL still carry every header the *Localhost Trust Boundary* requirement places on the shell, in every configuration. A conditional request whose tag does not match SHALL receive the full current content.

The report that the bundle must be built, and the not-found response for a missing path inside the static-asset namespace, SHALL forbid the browser from storing them in a cache. A bundle built after the server started then takes effect on the next load.

#### Scenario: A hashed asset is cached as immutable

- **WHEN** a browser requests a file under `assets/`
- **THEN** the response carries a caching policy marking it immutable for one year
- **AND** it is served with its own content type, as before

#### Scenario: The shell is revalidated on every load

- **WHEN** a browser requests the application shell at the root, at `/index.html` or at a deep address
- **THEN** the response requires revalidation before reuse and carries an entity tag
- **AND** it does not carry the immutable policy

#### Scenario: An unchanged shell is answered with not-modified and keeps its headers

- **WHEN** a browser requests the application shell with an `If-None-Match` matching the current shell's entity tag
- **THEN** the server responds with a not-modified status and no body
- **AND** the response carries `Content-Security-Policy: frame-ancestors 'none'`, `X-Frame-Options: DENY` and `X-DNS-Prefetch-Control: off`

#### Scenario: A rebuilt bundle is picked up on the next load

- **WHEN** the bundle has changed since the browser last loaded the shell, and the browser revalidates with the old entity tag
- **THEN** the server responds with the full new shell and a different entity tag

#### Scenario: Stable-path icons and the manifest are revalidated, not immutable

- **WHEN** a browser requests a declared icon, the favicon or the web app manifest
- **THEN** the response requires revalidation and carries an entity tag
- **AND** it does not carry the immutable policy

#### Scenario: The build hint is not cached

- **WHEN** no frontend bundle is present and a browser requests any path
- **THEN** the build-hint response forbids storing it in a cache

### Requirement: Compressed Responses Except the Event Stream

The web server SHALL compress response bodies with an encoding the client advertises in `Accept-Encoding`, at least gzip and Brotli, and SHALL mark such responses as varying by `Accept-Encoding`. This covers the bundle's scripts, stylesheets and shell, and the results of the command endpoint. A client that advertises no supported encoding SHALL receive the uncompressed body. A body too small to gain from compression MAY be sent uncompressed. Compression SHALL NOT change any response's status, content type or decoded body, and SHALL NOT change the command endpoint's wire shape or error envelope (see *Command Transport Mirrors the In-Process Command Surface*).

The event stream (see *Event Transport Reproduces the Frontend Event Contract*) SHALL NOT be compressed, so that each event reaches the browser when it is emitted rather than when a compressor flushes. Image types that are already compressed SHALL NOT be compressed again.

#### Scenario: A bundle script is compressed for a client that accepts it

- **WHEN** a browser that advertises Brotli or gzip requests a script under `assets/`
- **THEN** the response carries a matching `Content-Encoding` and `Vary: Accept-Encoding`
- **AND** the decoded body is byte-identical to the bundled file

#### Scenario: A client without compression gets the plain body

- **WHEN** a client that advertises no supported encoding requests a bundled asset or a command result
- **THEN** the response carries no `Content-Encoding`
- **AND** the body is the uncompressed content

#### Scenario: A command result is compressed without changing its shape

- **WHEN** a browser that advertises gzip dispatches a command whose result is larger than a trivial size, such as the workspace views or the dashboard
- **THEN** the response is compressed
- **AND** its decoded JSON is identical to the uncompressed result
- **AND** a failing command still decodes to the `{ "error": ... }` envelope with the same status

#### Scenario: The event stream is never compressed

- **WHEN** a browser that advertises Brotli and gzip opens the event stream
- **THEN** the stream response carries no `Content-Encoding`
- **AND** each event is delivered as it is emitted

#### Scenario: Images are not recompressed

- **WHEN** a browser that advertises gzip requests a raster icon
- **THEN** the response carries no `Content-Encoding`
