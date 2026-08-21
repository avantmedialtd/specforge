## ADDED Requirements

### Requirement: Served Document Declares an Icon Set

The served document SHALL declare an icon set in its head, so that a browser renders a SpecForge mark rather than a generic page glyph and never has to fall back to probing unlinked root paths.

The declared set SHALL include a scalable icon, a raster icon usable by consumers that do not accept the scalable form, and an Apple touch icon for home-screen installation. Every declared icon SHALL be a static asset of the bundle, reachable at a stable path that does not change between builds, because a consumer probing a well-known icon path never consults the bundle's module graph.

#### Scenario: The document declares its icons

- **WHEN** the served document is loaded from any origin the bundle is served from
- **THEN** its head declares a scalable icon, a raster icon, and an Apple touch icon
- **AND** each declared icon resolves to a bundled static asset

#### Scenario: Icon paths are stable across builds

- **WHEN** the frontend bundle is rebuilt
- **THEN** the paths of the declared icon assets are unchanged
- **AND** a request for a declared icon path is answered with that icon and its own content type

#### Scenario: A tab shows the product mark

- **WHEN** a browser opens the web UI in a tab
- **THEN** the tab displays the SpecForge mark
- **AND** the browser does not request an icon path that the application shell would answer

### Requirement: Small Sizes Use an Authored Glyph, Not the Illustration

The marks rendered at and below 32 px SHALL be an authored flat anvil glyph rather than a downscaling of the canonical application illustration, because the illustration's frame, hammer, sparks, and task-list detail do not survive that reduction. The marks rendered at 180 px and above SHALL be rasterizations of the canonical illustration, which is legible at those sizes and carries the full product identity.

The authored glyph SHALL be a distinct file from the tray glyphs described in the `tray-indicator` capability, so that the web mark and the macOS template images can change independently — see the *Canonical Application Icon Source* requirement in the `product-identity` capability.

#### Scenario: The small mark is recognizable at 16 px

- **WHEN** the browser renders the favicon at 16 px
- **THEN** the mark is recognizable as an anvil glyph
- **AND** it is not a downscaled rasterization of the canonical illustration

#### Scenario: Large marks carry the illustration

- **WHEN** an icon is rendered at 180 px or above
- **THEN** it presents the canonical forge illustration

#### Scenario: The web glyph is independent of the tray glyphs

- **WHEN** the repository's icon sources are enumerated
- **THEN** the authored web glyph is a different file from `tray-icon.svg` and `tray-specs.svg`
- **AND** editing the web glyph leaves both tray glyphs unchanged

### Requirement: Web App Manifest Is Origin-Agnostic

The bundle SHALL serve a web app manifest declaring the application name, a short name, a standalone display mode, and its icon set. The manifest's start URL and scope SHALL be expressed relatively, so that they resolve against the document that references them.

One build of the bundle is served from more than one origin — a loopback address on a configurable port, and a Tailscale name reached through an external proxy. A manifest whose start URL is absolute is correct on exactly one of those origins and wrong on every other, so the manifest SHALL NOT name any origin, host, or port.

#### Scenario: The manifest names no origin

- **WHEN** the served manifest is read
- **THEN** its start URL and scope are relative
- **AND** neither names a host, an origin, or a port

#### Scenario: The same manifest is correct on loopback and on a tailnet name

- **WHEN** the bundle is installed from a loopback address, and separately from a Tailscale name
- **THEN** each installation's start URL resolves to that installation's own origin
- **AND** neither resolves to the other's origin

#### Scenario: The manifest is served as a bundled asset

- **WHEN** a browser requests the manifest path
- **THEN** the server responds with the manifest and its own content type
- **AND** it does not respond with the application shell

### Requirement: Installed App Presents Its Own Icon and Window

When the served bundle is added to an iOS home screen, it SHALL appear with the SpecForge icon and SHALL open in its own window without browser chrome, rather than as a bookmark opening a browser tab.

The Apple touch icon SHALL be an opaque square raster with no alpha channel and no pre-applied corner rounding, because the platform composites transparency onto black and applies its own mask. The canonical application illustration already satisfies this, so the touch icon is a direct rasterization requiring no compositing step.

#### Scenario: The home-screen entry shows the product icon

- **WHEN** the user adds the served web UI to the iOS home screen
- **THEN** the home-screen entry displays the SpecForge icon
- **AND** it is labelled with the application's short name

#### Scenario: The installed app opens in its own window

- **WHEN** the user opens the installed entry from the home screen
- **THEN** the application opens in a standalone window without browser chrome
- **AND** it appears as its own entry in the app switcher

#### Scenario: The touch icon carries no transparency

- **WHEN** the Apple touch icon asset is decoded
- **THEN** it is opaque across its whole canvas
- **AND** its corners are square, with no rounding pre-applied

### Requirement: Icon Set Serves Masked Installers

The manifest SHALL declare a maskable icon in addition to the full-bleed icons, in which the illustration is inset within a solid field so that the safe area survives a circular or squircle crop.

The full-bleed icons SHALL NOT be declared as maskable, because the illustration's frame runs edge to edge and a mask would crop it. Declaring no maskable icon at all is likewise insufficient, because an installer that finds none composites the full-bleed square onto its own backdrop, producing a visible frame within a frame.

#### Scenario: A maskable icon is declared

- **WHEN** the served manifest's icon list is read
- **THEN** it contains an icon declared for masked use
- **AND** it also contains full-bleed icons that are not declared for masked use

#### Scenario: The maskable icon survives a circular crop

- **WHEN** the maskable icon is cropped to the platform's safe area
- **THEN** the forge illustration remains fully within the cropped region
- **AND** no part of the illustration's frame is cut by the crop

### Requirement: Theme and Launch Colours Come From the Design Tokens

The document SHALL declare theme colours for both colour schemes, taking their values from the background tokens the `visual-identity` capability defines rather than from independently chosen literals, so that browser chrome matches the application's own background in each scheme.

The manifest carries a single theme colour and a single background colour and therefore cannot vary by scheme; both SHALL take the dark-scheme value, because the background colour paints the launch surface before the bundle has rendered and the application's icon and identity are dark-field.

#### Scenario: Chrome matches the active colour scheme

- **WHEN** the served document is loaded in a browser set to the light colour scheme, and separately in one set to the dark colour scheme
- **THEN** the declared theme colour in each case matches the application's own background for that scheme

#### Scenario: Theme colours are not independent literals

- **WHEN** the declared theme colours are compared with the background tokens defined by the `visual-identity` capability
- **THEN** each declared colour equals the corresponding token's value

#### Scenario: The launch surface is dark

- **WHEN** an installed application is launched and the bundle has not yet rendered
- **THEN** the launch surface is painted with the dark-scheme background colour

### Requirement: Installability Adds No Service Worker

The bundle SHALL NOT register a service worker, and SHALL NOT precache or serve its own assets from a client-side cache.

The entire content of the UI comes from a live local server; an offline shell would present state that cannot be refreshed while appearing operational, which is a worse failure than being unavailable. The accepted cost is that installers which require a service worker before offering an install prompt will not offer one; home-screen installation on iOS does not require one and is unaffected.

#### Scenario: No service worker is registered

- **WHEN** the served bundle is loaded and has finished initializing
- **THEN** no service worker is registered for its scope
- **AND** no client-side cache serves the bundle's assets

#### Scenario: A suspended install does not present cached state as live

- **WHEN** an installed application is resumed and the server is unreachable
- **THEN** the application does not render a cached shell as though it were current
- **AND** it surfaces that state could not be read — see the *Event Stream Recovers After Document Suspension* requirement in the `web-ui` capability

### Requirement: Installability Never Requires the Server to Terminate TLS

Installability SHALL be a property of how the server is reached, never a reason for the server to bind a network interface or terminate TLS itself. The server SHALL continue to bind loopback by default and to serve plain HTTP, exactly as the `web-ui` capability specifies.

A secure context — which installation requires — is supplied by the loopback origin itself, or by the external `tailscale serve` proxy that terminates TLS in front of the loopback port. Where the bundle is reached over an explicitly requested non-loopback bind on plain HTTP, installation degrades to a plain home-screen bookmark; this is an accepted limitation of that access path and SHALL NOT be addressed by adding TLS to the server.

#### Scenario: No TLS is added to the server

- **WHEN** the icon and manifest assets are served
- **THEN** the server terminates no TLS itself
- **AND** it binds no interface it would not otherwise have bound

#### Scenario: A proxied tailnet origin is installable

- **WHEN** the bundle is reached through the external proxy at the host's Tailscale name over HTTPS
- **THEN** the origin is a secure context and the bundle is installable

#### Scenario: A plain-HTTP network bind degrades rather than escalating

- **WHEN** the bundle is reached over an explicitly requested non-loopback bind on plain HTTP
- **THEN** installation degrades to a home-screen bookmark
- **AND** the server does not begin terminating TLS in response

### Requirement: The Install Surface Is Inert in the Desktop Shell

The desktop application loads the same document as the browser skin. The icon declarations, manifest reference, and theme-colour declarations SHALL be harmless there, and the desktop shell SHALL NOT gain an install affordance, a tab icon, or any behaviour change from their presence.

#### Scenario: Desktop behaviour is unchanged by the install markup

- **WHEN** the desktop application loads the shared document
- **THEN** its window, tray, and menus behave exactly as they did before the install markup existed
- **AND** no install affordance is presented
