# marketing-site

## REMOVED Requirements

### Requirement: Downloads link the latest release, never a version

**Reason**: the prohibition made a download link impossible to construct. Every
release asset embeds its version in its filename, so a page forbidden from
naming a version could only describe artefacts rather than link them. The
freshness the requirement protected is now enforced by the *Site Version Matches
Tag* requirement in the `release-pipeline` capability, which fails a release
whose site constant does not match the tag.

## ADDED Requirements

### Requirement: Downloads name the current release and link its assets

The download block SHALL name the release it is offering and SHALL link each
advertised artefact directly to that release's asset URL.

It SHALL advertise the macOS universal `.dmg` (macOS 11.0+), the Windows NSIS
installer and portable `.exe` (x64), the Linux `.deb` and `.AppImage` (x64), the
standalone terminal-UI archives, and the standalone `specforge-serve` archives
including `linux-arm64`. Unsigned-build caveats SHALL appear inline and link to
`/docs/troubleshooting`.

Every control styled as a download SHALL resolve to an asset. A control that
merely navigates — to the releases page, the documentation or the package
registry — SHALL be visually distinct from one that downloads.

The version SHALL be rendered from a single constant in the site's
configuration, never fetched at page load. See the *No cookies and no analytics*
requirement.

#### Scenario: Every advertised artefact links its asset

- **WHEN** the download block renders
- **THEN** each advertised artefact SHALL carry a link to that artefact's release asset URL
- **AND** the `specforge-serve` platform list SHALL include `linux-arm64`

#### Scenario: The offered release is named

- **WHEN** the download block renders
- **THEN** it SHALL state the version it is offering

#### Scenario: Navigation is not dressed as a download

- **WHEN** the download block renders
- **THEN** no control that only navigates SHALL carry the styling of a download control

### Requirement: The download block sits above the fold

The landing page SHALL present a working download and the no-download npm route
without scrolling, on a viewport of 1440x900.

The hero's primary action SHALL download a file rather than scroll the page, and
the landing page SHALL NOT render a strip of platform labels styled as controls
but carrying no behaviour.

The site header's download link is exempt: it must resolve from every route, so
on the landing page it necessarily targets a section of that page. It is styled
as navigation, not as a download.

#### Scenario: A first-time visitor can download without scrolling

- **WHEN** the landing page is rendered at 1440x900
- **THEN** a download control and the npm command SHALL both be visible without scrolling

#### Scenario: The hero's primary action acts rather than scrolls

- **WHEN** the landing page renders
- **THEN** the hero's primary action SHALL target a release asset or the releases page
- **AND** it SHALL NOT target an anchor on the same page

#### Scenario: Nothing that looks like a control is inert

- **WHEN** the landing page renders
- **THEN** every element styled as a control SHALL carry a link

### Requirement: The primary download follows the visitor's platform

The server-rendered HTML SHALL carry a platform-neutral download control
targeting the repository's releases page. After hydration the control SHALL be
relabelled and retargeted to the detected platform's asset.

Detection SHALL read only information the browser already holds and SHALL issue
no request. Every other platform's asset SHALL remain reachable without
JavaScript.

#### Scenario: Detection leads with the visitor's platform

- **WHEN** the page hydrates and the visitor's platform is recognised
- **THEN** the primary control SHALL name that platform and link its asset

#### Scenario: The control works without JavaScript

- **WHEN** the page is rendered with scripting unavailable
- **THEN** the primary control SHALL still resolve to a working download route
- **AND** every platform's asset SHALL remain reachable from the page

#### Scenario: Detection calls nobody

- **WHEN** a visitor loads the landing page
- **THEN** platform detection SHALL issue no network request
