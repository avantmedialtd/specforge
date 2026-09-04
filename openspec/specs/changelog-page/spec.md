# changelog-page Specification

## Purpose
TBD - created by archiving change add-changelog-page. Update Purpose after archive.
## Requirements
### Requirement: The changelog route renders the current release

The site SHALL serve a `/changelog` route that prerenders to a static HTML document carrying a single `<h1>`. The route SHALL render, in full, the release named by the site's advertised version constant, sourced from that release's authored notes file rather than from copy duplicated into the site.

The rendered release SHALL show its version, its release date, and its authored standfirst, followed by the note's own sections in their authored order.

#### Scenario: The advertised release is the one rendered

- **WHEN** the site is built with its version constant set to `0.21.0`
- **THEN** `/changelog` SHALL render the contents of `releases/v0.21.0.md`
- **AND** the page SHALL carry exactly one `<h1>`

#### Scenario: A missing notes file fails the build

- **WHEN** the version constant names a release for which no notes file exists
- **THEN** the build SHALL fail and name the missing file
- **AND** no page SHALL be emitted with an empty or placeholder release

#### Scenario: The route is reachable and indexed

- **WHEN** a visitor navigates to `/changelog`
- **THEN** the page SHALL be served with status 200
- **AND** it SHALL appear in `sitemap.xml`, as required by the *Product routes* requirement in the `marketing-site` capability

### Requirement: Release notes are adapted for the web, not shipped as authored

Release notes are authored for GitHub Releases, where the release page is also the download page. The site SHALL adapt them at render time rather than change their format, since the same files are consumed verbatim as GitHub Release bodies — see the *Versioned Notes File* requirement in the `release-command` capability.

The site SHALL discard each note's Downloads footer, which duplicates the site's own download block and names version-pinned artefacts that go stale on a page describing an older release. The site SHALL promote each note's opening version line — authored as plain text, carrying no heading marker — to a real heading, and SHALL render the paragraph that follows it as a standfirst rather than as body copy.

#### Scenario: The Downloads footer is not published

- **WHEN** a release note is rendered on the changelog page
- **THEN** the content from its Downloads heading onward SHALL NOT appear
- **AND** no artefact filename, install command or Full Changelog link from that footer SHALL appear on the page

#### Scenario: The version line becomes a heading

- **WHEN** a note whose first line is plain text naming the release is rendered
- **THEN** that line SHALL be emitted as the release's heading rather than as a paragraph
- **AND** it SHALL NOT also appear in the rendered body

#### Scenario: A note without a standfirst still renders

- **WHEN** a note goes straight from its version line to its first section heading
- **THEN** the release SHALL render with no standfirst
- **AND** the build SHALL NOT fail

### Requirement: The cut contract is asserted, not assumed

The boundary between a note's changelog and its Downloads footer is a contract between two things that do not otherwise know about each other: the notes are machine-authored by the release command, and the site pattern-matches their footer heading. The build SHALL assert that contract on every note it renders rather than degrade silently when it no longer holds.

#### Scenario: A note with no cut point fails the build

- **WHEN** a note the page renders carries no Downloads heading
- **THEN** the build SHALL fail and name the offending file
- **AND** the site SHALL NOT publish that note's install instructions as changelog copy

### Requirement: Earlier releases are listed in condensed form

Below the current release the page SHALL list earlier releases newest-first, each as its version, its date, and its authored standfirst, linking to that release on the repository host. The page SHALL NOT render earlier releases in full: the complete corpus is an hour's reading and would ship on every visit.

Prerelease notes SHALL be excluded from the list, being near-duplicates of the final release that follows them.

#### Scenario: Earlier releases are summarised, not rendered

- **WHEN** the changelog page renders
- **THEN** each earlier release SHALL contribute its version, date and standfirst
- **AND** no section headings or bullet lists from an earlier release SHALL appear

#### Scenario: Prereleases are omitted

- **WHEN** the notes directory contains both a prerelease and its final release
- **THEN** only the final release SHALL be listed

### Requirement: Markdown conversion never reaches the visitor

The site converts markdown only while building. A markdown parser SHALL NOT appear in the site's runtime dependencies and SHALL NOT be reachable from the client bundle, preserving the small, deliberate dependency set the site carries.

#### Scenario: The runtime dependency set is unchanged

- **WHEN** the changelog page ships
- **THEN** the site's runtime dependencies SHALL be exactly those it carried before
- **AND** any markdown library SHALL be a development dependency only

#### Scenario: No parser is served to the browser

- **WHEN** a visitor loads `/changelog`
- **THEN** the page SHALL arrive as prerendered HTML
- **AND** no markdown parsing SHALL occur in the browser

### Requirement: Rendered notes carry the site's own typography

Machine-generated markup arrives without utility classes, and the site's base layer zeroes heading sizes and list markers. The site SHALL style rendered notes with its own first-party rules, written against its existing design tokens and placed in a cascade layer so they do not outrank utilities. It SHALL NOT import an external typography theme, as required by the *Site carries its own visual identity* requirement in the `marketing-site` capability.

Notes authored across different eras vary between tight and loose list spacing. The page SHALL present one consistent spacing regardless of which era a note was authored in.

#### Scenario: Generated headings are styled

- **WHEN** a rendered note emits section headings
- **THEN** they SHALL be visually distinguishable from body copy in size or weight
- **AND** their styling SHALL derive from the site's own tokens

#### Scenario: List spacing does not vary by release era

- **WHEN** notes authored with tight and loose list markup are rendered on the same page
- **THEN** their bullet spacing SHALL be identical

#### Scenario: No external theme is introduced

- **WHEN** the site is built
- **THEN** its stylesheet SHALL resolve entirely from within the site directory

### Requirement: Generated heading ids are unique across releases

Release notes reuse a small set of section headings, so a page carrying several releases would otherwise emit many elements sharing one id, breaking deep links and violating document uniqueness. Every id the page generates SHALL be unique within the document and SHALL remain stable across builds so that a published anchor keeps working.

#### Scenario: Repeated section names do not collide

- **WHEN** two releases on the page both carry a section of the same name
- **THEN** their headings SHALL receive different ids

#### Scenario: An anchor clears the sticky header

- **WHEN** a visitor follows a link to a generated heading id
- **THEN** the target SHALL come to rest below the site header rather than beneath it

### Requirement: The changelog is reachable at every viewport width

The site SHALL link the changelog from every page. The header is the primary
route to it, but the header row cannot carry a fourth destination on a narrow
phone: the additional item overflows the smallest supported width and wraps the
navigation onto a second line, which grows the sticky header past the scroll
offset every in-page anchor depends on and breaks the shared vertical centre the
brand, navigation and download control hold.

Where the header cannot show the link, the footer SHALL carry it, so no viewport
loses access to the page. The footer link SHALL NOT be added to the documentation
navigation array, which drives the docs sidebar as well and would file the
release history as reference material.

#### Scenario: A wide viewport shows the header link

- **WHEN** a visitor loads any page at a desktop width
- **THEN** the primary navigation SHALL show a visible link to the changelog

#### Scenario: A narrow viewport keeps the page reachable

- **WHEN** a visitor loads any page at a phone width
- **THEN** the primary navigation SHALL NOT show the changelog link
- **AND** the footer navigation SHALL show it

#### Scenario: The header still fits its smallest supported width

- **WHEN** any page is rendered at the smallest supported viewport width
- **THEN** the document SHALL NOT scroll horizontally
- **AND** the sticky header SHALL remain no taller than the anchor scroll offset

### Requirement: Release prose is never asserted verbatim by the test suite

The changelog's content changes on every release and is authored by the release command, not by the site. The site's test suite SHALL assert the page's structure and never its wording, so that shipping a release does not require editing a test. Content guards written for site-authored copy SHALL NOT be applied to the changelog route.

#### Scenario: A new release does not break the suite

- **WHEN** a release adds a notes file and moves the version constant
- **THEN** the site's tests SHALL pass without modification

#### Scenario: The page is still guarded against being empty

- **WHEN** the changelog page renders
- **THEN** the suite SHALL assert that the current release's version appears and that at least one rendered section is present
- **AND** it SHALL derive that version from the site's configuration rather than from a literal

#### Scenario: Copy guards do not police authored release notes

- **WHEN** a content guard scans site-authored copy across routes
- **THEN** the changelog route SHALL be excluded, with the reason recorded where the exclusion is made

