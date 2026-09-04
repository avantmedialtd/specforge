/**
 * The origin this site is served from. Structured data needs absolute URLs, and
 * `render-config.ts` reads it so the canonical host is declared in one place.
 */
export const SITE_URL = 'https://specforge.avantmedia.uk';

/** The product repository. Every download and source link derives from this. */
export const REPO_URL = 'https://github.com/avantmedialtd/specforge';

/**
 * The release this site advertises.
 *
 * `/release` step 8 writes this into the same commit as `releases/<tag>.md`, so
 * the push that tags a release also matches `site.yml`'s `site/` path filter and
 * redeploys the site. It cannot be written by `scripts/bump-version.ts`: that
 * script tags a commit that already exists, so it runs *after* the commit this
 * constant has to be part of.
 *
 * `release.yml`'s `check-site-version` job asserts this equals the tag being
 * released, and runs before the platform builds — a forgotten bump fails in
 * seconds rather than after twenty minutes of building.
 *
 * Do not edit by hand outside a release.
 */
export const RELEASE_VERSION = '0.21.0';

/** The git tag for {@link RELEASE_VERSION}. */
export const RELEASE_TAG = `v${RELEASE_VERSION}`;

/** The release page for the version this site advertises. */
export const RELEASE_URL = `${REPO_URL}/releases/tag/${RELEASE_TAG}`;

/**
 * The releases landing page. Still the right target for "every release" links
 * and for the no-JavaScript download fallback, which must resolve without
 * knowing the visitor's platform.
 */
export const LATEST_RELEASE_URL = `${REPO_URL}/releases/latest`;

/** A direct download URL for one asset of the advertised release. */
export function assetUrl(file: string): string {
    return `${REPO_URL}/releases/download/${RELEASE_TAG}/${file}`;
}

/** The three operating systems the desktop app ships bundles for. */
export type Platform = 'macos' | 'windows' | 'linux';

export interface DownloadItem {
    /** Asset filename within the release, version already substituted. */
    file: string;
    /** Which operating system this artefact is for. */
    label: string;
    /** Format and architecture, as the visitor needs to read it. */
    detail: string;
    /**
     * Set when this is the artefact to lead with for a detected platform. Only
     * the desktop bundles carry it — the terminal and server archives are
     * secondary routes, never the hero's primary control.
     */
    lead?: Platform;
}

export interface DownloadGroup {
    id: string;
    title: string;
    blurb: string;
    items: DownloadItem[];
}

/**
 * Every asset the release publishes, grouped as the download block renders them.
 * The filename patterns mirror `release.yml`'s bundle naming and the Downloads
 * footer in `.claude/commands/release.md`; the two are expected to move together.
 */
export const DOWNLOAD_GROUPS: DownloadGroup[] = [
    {
        id: 'desktop',
        title: 'Desktop app',
        blurb: 'Tray badge and dock indicator, one click from the menu bar.',
        items: [
            {
                file: `SpecForge_${RELEASE_VERSION}_universal.dmg`,
                label: 'macOS',
                detail: 'universal .dmg · macOS 11.0+',
                lead: 'macos',
            },
            {
                file: `SpecForge_${RELEASE_VERSION}_x64-setup.exe`,
                label: 'Windows',
                detail: 'NSIS installer · x64',
                lead: 'windows',
            },
            {
                file: `SpecForge_${RELEASE_VERSION}_x64-portable.exe`,
                label: 'Windows',
                detail: 'portable .exe · x64',
            },
            {
                file: `SpecForge_${RELEASE_VERSION}_amd64.deb`,
                label: 'Linux',
                detail: '.deb · x64',
                lead: 'linux',
            },
            {
                file: `SpecForge_${RELEASE_VERSION}_amd64.AppImage`,
                label: 'Linux',
                detail: '.AppImage · x64',
            },
        ],
    },
    {
        id: 'tui',
        title: 'Terminal UI',
        blurb: 'Interactive browser, one-shot --status and a compact --line for your prompt.',
        items: [
            {
                file: `specforge-tui_${RELEASE_VERSION}_macos-universal.tar.gz`,
                label: 'macOS',
                detail: 'universal · .tar.gz',
            },
            {
                file: `specforge-tui_${RELEASE_VERSION}_linux-x64.tar.gz`,
                label: 'Linux',
                detail: 'x64 · .tar.gz',
            },
            {
                file: `specforge-tui_${RELEASE_VERSION}_windows-x64.zip`,
                label: 'Windows',
                detail: 'x64 · .zip',
            },
        ],
    },
    {
        id: 'serve',
        title: 'Local web server',
        blurb: 'The browser UI on localhost, a headless machine, or through a tunnel.',
        items: [
            {
                file: `specforge-serve_${RELEASE_VERSION}_macos-universal.tar.gz`,
                label: 'macOS',
                detail: 'universal · .tar.gz',
            },
            {
                file: `specforge-serve_${RELEASE_VERSION}_linux-x64.tar.gz`,
                label: 'Linux',
                detail: 'x64 · .tar.gz',
            },
            {
                file: `specforge-serve_${RELEASE_VERSION}_linux-arm64.tar.gz`,
                label: 'Linux',
                detail: 'arm64 · .tar.gz',
            },
            {
                file: `specforge-serve_${RELEASE_VERSION}_windows-x64.zip`,
                label: 'Windows',
                detail: 'x64 · .zip',
            },
        ],
    },
];

/** The desktop bundle to lead with once a platform has been detected. */
export const LEAD_DOWNLOADS: Record<Platform, DownloadItem> = Object.fromEntries(
    DOWNLOAD_GROUPS[0].items.filter(i => i.lead).map(i => [i.lead as Platform, i]),
) as Record<Platform, DownloadItem>;

/** How each platform is named in the primary control's label. */
export const PLATFORM_NAMES: Record<Platform, string> = {
    macos: 'macOS',
    windows: 'Windows',
    linux: 'Linux',
};

/** The studio that builds SpecForge, and its booking page. */
export const STUDIO_URL = 'https://www.avantmedia.uk';
export const BOOKING_URL = 'https://cal.eu/istvan';

/** The OpenSpec format SpecForge reads. */
export const OPENSPEC_URL = 'https://github.com/Fission-AI/OpenSpec';

/**
 * The npm package that publishes the web UI (`specforge-serve`). Named
 * without a version on purpose: `npx` resolves the newest release itself, and
 * an `e2e` test fails the build if any install command pins one.
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
