# marketing-site Specification

## ADDED Requirements

### Requirement: Site lives in this repository and serves its own domain

The repository SHALL host the SpecForge marketing site at `site/`, a Vike application deployed to `https://specforge.avantmedia.uk`. It SHALL declare `<html lang="en">`, root every absolute URL in its rendered head at that origin, and SHALL NOT reference an Avant Media studio domain in any canonical, Open Graph, or sitemap URL.

`site/` SHALL be standalone: it carries its own `package.json`, lockfile and `tsconfig.json`, and SHALL NOT be a member of any workspace glob. The desktop app's root `bun install`, root type-check and root build SHALL be unaffected by its presence.

#### Scenario: Site builds to its own static root

- **WHEN** `bun run --cwd site build` is run
- **THEN** `site/dist/client/` contains the site's static HTML, CSS, JS and assets
- **AND** it contains no page belonging to another site

#### Scenario: The desktop app's tooling is unaffected

- **WHEN** `bun install --frozen-lockfile`, `bun run build` and `bun test` are run at the repository root
- **THEN** each succeeds with no lockfile change
- **AND** `bun test` discovers exactly the files it discovered before the site existed

#### Scenario: Canonical URLs use the SpecForge domain

- **WHEN** any page is rendered
- **THEN** its canonical and `og:url` SHALL be rooted at `https://specforge.avantmedia.uk`

### Requirement: Product routes

The site SHALL provide nine routes: `/`, `/docs`, `/docs/workspaces`, `/docs/dashboard`, `/docs/commit-graph`, `/docs/terminal-ui`, `/docs/web-ui`, `/docs/settings`, and `/docs/troubleshooting`. Each SHALL prerender to a static HTML document and appear in the site's `sitemap.xml`.

A prerendered 404 document SHALL also be emitted. It is not a route: it carries `noindex`, is excluded from the sitemap, and is reached only by requesting a path that does not exist.

The site publishes no feed — it has no dated articles — so `/feed.xml` SHALL NOT exist.

#### Scenario: Every route is reachable

- **WHEN** a visitor navigates to any of the nine routes
- **THEN** the page is served with status 200 and renders its own H1

#### Scenario: Every route is in the sitemap

- **WHEN** the site is built
- **THEN** `site/dist/client/sitemap.xml` contains a `<loc>` for each of the nine routes and nothing else
- **AND** it contains no `<loc>` for the 404 document

#### Scenario: Adding a page without a date fails the build

- **WHEN** a page is added whose `+documentProps.ts` carries no `modified` or `date`
- **THEN** the build SHALL fail rather than emit a sitemap entry with an unauthored `<lastmod>`

### Requirement: Landing page states the product's positioning

The landing page SHALL open with an H1 stating what SpecForge does for spec-driven work. It SHALL currently read "Spec-driven work, in full view." The word "viewer" SHALL NOT be used to describe the product in site copy, notwithstanding its use in the product README.

#### Scenario: Landing page carries its headline

- **WHEN** the landing page renders
- **THEN** its H1 SHALL read "Spec-driven work, in full view."
- **AND** the meta description SHALL carry the same framing

### Requirement: Read-only is framed as a design choice

The site SHALL frame SpecForge's read-only posture as a deliberate design choice, in a dedicated section stating that it never edits specs, never toggles checkboxes and never touches git. That framing SHALL NOT be hedged with "yet", "for now" or "v1 only".

#### Scenario: The read-only section reads as intent, not limitation

- **WHEN** the landing page renders
- **THEN** a section SHALL describe the read-only posture as a design choice
- **AND** it SHALL contain no hedging language implying the posture is temporary

### Requirement: Downloads link the latest release, never a version

Every download link SHALL target the repository's `releases/latest`, never a versioned asset URL. No rendered page SHALL name a version number, so the site cannot go stale between releases.

The download block SHALL name the macOS universal `.dmg` (macOS 11.0+), the Windows NSIS installer and portable `.exe` (x64), the Linux `.deb` and `.AppImage` (x64), the standalone terminal-UI archives, and the standalone `specforge-serve` archives including `linux-arm64`. Unsigned-build caveats SHALL appear inline and link to `/docs/troubleshooting`.

#### Scenario: No page names a version

- **WHEN** any page is rendered
- **THEN** no rendered text SHALL match a semantic version pattern
- **AND** no link SHALL target a `/releases/download/` asset URL

#### Scenario: Every platform artefact is advertised

- **WHEN** the downloads section renders
- **THEN** it SHALL name each supported bundle format and platform
- **AND** the `specforge-serve` platform list SHALL include `linux-arm64`

### Requirement: The npm channel is offered as the no-download route

The downloads block and `/docs/web-ui` SHALL offer `@avantmedia/specforge` as a route requiring no download and no quarantine step. The package SHALL always be rendered **scoped** — the unscoped `specforge` on the public registry belongs to an unrelated project — and no rendered install command SHALL pin a version.

#### Scenario: The npm route is always scoped

- **WHEN** any page renders an npm install or execute command
- **THEN** it SHALL name `@avantmedia/specforge`
- **AND** it SHALL NOT render an unscoped `specforge` install or execute command

### Requirement: Both usage-quota gauges are documented accurately

`/docs/settings` SHALL document the Claude and ChatGPT usage gauges: each off by default, each reading its local CLI login read-only, and the two independent of one another. Network calls SHALL be described in the plural, and the documentation SHALL state that with both gauges off the product makes no network call at all.

#### Scenario: Quota documentation matches the product

- **WHEN** `/docs/settings` renders
- **THEN** it SHALL describe both gauges as independently opt-in
- **AND** it SHALL state that with both off, no network call is made

### Requirement: Troubleshooting covers every unsigned-build path

`/docs/troubleshooting` SHALL cover each way an unsigned build is blocked: macOS Gatekeeper (right-click ▸ Open), Windows SmartScreen (More info → Run anyway), WebView2 for the portable `.exe`, the Linux `.deb` and `.AppImage`, and `xattr -dr com.apple.quarantine` for the standalone terminal-UI and `specforge-serve` binaries, which have no right-click ▸ Open affordance.

#### Scenario: Each blocked path has a documented remedy

- **WHEN** `/docs/troubleshooting` renders
- **THEN** it SHALL document a remedy for each unsigned-build path above

### Requirement: No cookies and no analytics

The site SHALL set no cookies, load no tracking script, and make no third-party request. It therefore SHALL NOT carry a cookie banner or a privacy page. Fonts SHALL be self-hosted rather than fetched from a font CDN.

#### Scenario: A visit sets nothing and calls nobody

- **WHEN** a visitor loads any page
- **THEN** no cookie SHALL be set
- **AND** every request SHALL be same-origin or a `data:` URI

### Requirement: Site carries its own visual identity

The site SHALL carry a visual identity derived from the SpecForge desktop app's own stylesheet, not from a shared design system. It SHALL import no external component library or theme, and SHALL use its own favicons and its own Open Graph image rather than a studio asset.

Studio attribution SHALL be understated: a closing "built by Avant Media" of no more than two sentences, linking the studio site, with no services pitch and no additional call to action.

#### Scenario: The stylesheet is self-contained

- **WHEN** the site is built
- **THEN** its stylesheet SHALL resolve entirely from within `site/`
- **AND** the rendered head SHALL reference the site's own icon and Open Graph assets

#### Scenario: Attribution stays understated

- **WHEN** any page renders
- **THEN** studio attribution SHALL be at most two sentences
- **AND** it SHALL NOT present a services offer

### Requirement: Shared rendering code is vendored, not depended on

The document-meta and build-time discovery code the site shares in origin with the Avant Media studio sites SHALL live in `site/site-kit/` as first-party source, imported by relative path. No module under `site/site-kit/` SHALL import a package belonging to a studio design system, and no barrel re-exporting studio chrome SHALL be introduced.

`site/site-kit/README.md` SHALL record the commit the fork was taken at and every deviation from it.

#### Scenario: The vendored tree reaches nothing external

- **WHEN** the site is built or type-checked
- **THEN** every import under `site/site-kit/` SHALL resolve within `site/` or to a declared dependency
- **AND** no import SHALL name a studio design-system package

### Requirement: Publishing is path-filtered, gated and explicitly armed

A dedicated GitHub Actions workflow SHALL build, type-check, spell-check and test the site, and SHALL publish it to the S3 bucket serving `specforge.avantmedia.uk` followed by a CloudFront invalidation. It SHALL run only for changes touching `site/` or the workflow itself, and SHALL authenticate by OIDC role assumption rather than long-lived credentials.

The workflow SHALL NOT modify `ci.yml`, whose trigger and jobs are fixed by the *Pipeline Trigger* and *Parallel Job Execution* requirements in the `continuous-integration` capability.

Publishing SHALL be doubly gated: the deploy job SHALL be skipped unless a deploy role ARN is configured, and SHALL perform a dry run unless publishing is explicitly set to live. Before any `--delete` sync, the workflow SHALL assert that the built artefact contains a set of required documents and meets a minimum file count.

#### Scenario: The pipeline lands inert

- **WHEN** the workflow runs on the default branch with no deploy role configured
- **THEN** the build and test job SHALL run
- **AND** the deploy job SHALL be skipped without failing the workflow

#### Scenario: A thin artefact is never published

- **WHEN** the built artefact is missing a required document or falls below the minimum file count
- **THEN** the workflow SHALL fail before the sync
- **AND** no object SHALL be written to or deleted from the bucket

#### Scenario: A deployed site is verified

- **WHEN** a live publish completes
- **THEN** every URL in the deployed sitemap SHALL be requested and SHALL return 200
- **AND** paths belonging to the studio site SHALL return 404

### Requirement: Page copy uses British English

Rendered page copy SHALL use British `-ise`/`-isation`/`-yse` forms. A build check SHALL fail on American spellings in the site's `pages/` and `src/` trees. The schema.org `Organization` type and identifiers derived from it are excluded, being API identifiers rather than prose.

#### Scenario: An American spelling fails the build

- **WHEN** page or component copy contains a banned American form
- **THEN** the British-English check SHALL fail and name the file and line
