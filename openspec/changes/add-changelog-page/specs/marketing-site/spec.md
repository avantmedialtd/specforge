## MODIFIED Requirements

### Requirement: Product routes

The site SHALL provide ten routes: `/`, `/changelog`, `/docs`, `/docs/workspaces`, `/docs/dashboard`, `/docs/commit-graph`, `/docs/terminal-ui`, `/docs/web-ui`, `/docs/settings`, and `/docs/troubleshooting`. Each SHALL prerender to a static HTML document and appear in the site's `sitemap.xml`.

`/changelog` is a top-level route rather than a documentation page: its audience includes visitors who are not reading the documentation, and its content is authored by the release command rather than by the site. Its behaviour is defined by the `changelog-page` capability.

A prerendered 404 document SHALL also be emitted. It is not a route: it carries `noindex`, is excluded from the sitemap, and is reached only by requesting a path that does not exist.

The site publishes no feed — it has no dated articles — so `/feed.xml` SHALL NOT exist.

#### Scenario: Every route is reachable

- **WHEN** a visitor navigates to any of the ten routes
- **THEN** the page is served with status 200 and renders its own H1

#### Scenario: Every route is in the sitemap

- **WHEN** the site is built
- **THEN** `site/dist/client/sitemap.xml` contains a `<loc>` for each of the ten routes and nothing else
- **AND** it contains no `<loc>` for the 404 document

#### Scenario: Adding a page without a date fails the build

- **WHEN** a page is added whose `+documentProps.ts` carries no `modified` or `date`
- **THEN** the build SHALL fail rather than emit a sitemap entry with an unauthored `<lastmod>`

### Requirement: Publishing is path-filtered, gated and explicitly armed

A dedicated GitHub Actions workflow SHALL build, type-check, spell-check and test the site, and SHALL publish it to the S3 bucket serving `specforge.avantmedia.uk` followed by a CloudFront invalidation. It SHALL run for changes touching `site/`, the release notes the site renders, or the workflow itself, and SHALL authenticate by OIDC role assumption rather than long-lived credentials.

The notes directory is a trigger path because the site now renders content from outside `site/`: without it, correcting a published note would leave the deployed changelog stale until an unrelated site change happened to redeploy.

The workflow SHALL NOT modify `ci.yml`, whose trigger and jobs are fixed by the *Pipeline Trigger* and *Parallel Job Execution* requirements in the `continuous-integration` capability.

Publishing SHALL be doubly gated: the deploy job SHALL be skipped unless a deploy role ARN is configured, and SHALL perform a dry run unless publishing is explicitly set to live. Before any `--delete` sync, the workflow SHALL assert that the built artefact contains a set of required documents and meets a minimum file count.

#### Scenario: The pipeline lands inert

- **WHEN** the workflow runs on the default branch with no deploy role configured
- **THEN** the build and test job SHALL run
- **AND** the deploy job SHALL be skipped without failing the workflow

#### Scenario: A corrected release note redeploys the site

- **WHEN** a commit changes only a file under the release-notes directory
- **THEN** the site workflow SHALL run
- **AND** the republished changelog SHALL carry the corrected note

#### Scenario: A thin artefact is never published

- **WHEN** the built artefact is missing a required document or falls below the minimum file count
- **THEN** the workflow SHALL fail before the sync
- **AND** no object SHALL be written to or deleted from the bucket

#### Scenario: A deployed site is verified

- **WHEN** a live publish completes
- **THEN** every URL in the deployed sitemap SHALL be requested and SHALL return 200
- **AND** paths belonging to the studio site SHALL return 404
