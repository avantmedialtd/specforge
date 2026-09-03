/**
 * The origin this site is served from. Structured data needs absolute URLs, and
 * `render-config.ts` reads it so the canonical host is declared in one place.
 */
export const SITE_URL = 'https://specforge.avantmedia.uk';

/** The product repository. Every download and source link derives from this. */
export const REPO_URL = 'https://github.com/avantmedialtd/specforge';

/**
 * The releases landing page — deliberately `latest` rather than a versioned
 * asset URL, so the download block cannot go stale between releases. Nothing on
 * this site names a version number.
 */
export const LATEST_RELEASE_URL = `${REPO_URL}/releases/latest`;

/** The studio that builds SpecForge, and its booking page. */
export const STUDIO_URL = 'https://www.avantmedia.uk';
export const BOOKING_URL = 'https://cal.eu/istvan';

/** The OpenSpec format SpecForge reads. */
export const OPENSPEC_URL = 'https://github.com/Fission-AI/OpenSpec';

/**
 * The npm package that publishes the web UI (`specforge-serve`). Named
 * without a version on purpose: `npx` resolves the newest release itself, and
 * an `e2e` test fails the build if any version string reaches a rendered page.
 * The scope is load-bearing — the unscoped `specforge` on the public registry
 * belongs to an unrelated project.
 */
export const NPM_PACKAGE = '@avantmedia/specforge';
export const NPM_PACKAGE_URL = `https://www.npmjs.com/package/${NPM_PACKAGE}`;

export interface DocsNavItem {
    href: string;
    label: string;
}

/**
 * The documentation table of contents, in reading order. Drives both the header
 * nav and the in-page docs sidebar, so the two cannot drift.
 */
export const DOCS_NAV: DocsNavItem[] = [
    { href: '/docs', label: 'Getting started' },
    { href: '/docs/workspaces', label: 'Workspaces' },
    { href: '/docs/dashboard', label: 'Dashboard' },
    { href: '/docs/commit-graph', label: 'Commit graph' },
    { href: '/docs/terminal-ui', label: 'Terminal UI' },
    { href: '/docs/web-ui', label: 'Web UI' },
    { href: '/docs/settings', label: 'Settings' },
    { href: '/docs/troubleshooting', label: 'Troubleshooting' },
];
