import { expect, test } from '@playwright/test';
import { DOWNLOAD_GROUPS, RELEASE_TAG, RELEASE_VERSION } from '../../src/site-config';

const RELEASES_LATEST = 'https://github.com/avantmedialtd/specforge/releases/latest';
const DOWNLOAD_BASE = `https://github.com/avantmedialtd/specforge/releases/download/${RELEASE_TAG}`;

// Every asset the release publishes, in the order the page renders them.
const ASSET_FILES = DOWNLOAD_GROUPS.flatMap(group => group.items.map(item => item.file));

// Every public route, so the pinned-version guard below covers the whole site
// rather than only the page that carries the download block.
const ROUTES = [
    '/',
    '/docs',
    '/docs/workspaces',
    '/docs/dashboard',
    '/docs/commit-graph',
    '/docs/terminal-ui',
    '/docs/web-ui',
    '/docs/settings',
    '/docs/troubleshooting',
];

test.describe('Downloads', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
    });

    // This imports the same constant the page renders, so it deliberately does
    // NOT prove the version is current — that is `release.yml`'s
    // `check-site-version` job, which compares the constant against the tag
    // being released. What it proves is that every artefact the site advertises
    // resolves to a real asset URL rather than a description of one.
    // Scoped to the download block: the hero's primary control links one of
    // these assets too once detection has run, so an unscoped count would be 2
    // for whichever platform the test browser reports.
    test('every advertised artefact links its release asset', async ({ page }) => {
        expect(ASSET_FILES, 'the release publishes twelve assets').toHaveLength(12);
        const downloads = page.locator('#downloads');
        for (const file of ASSET_FILES) {
            await expect(
                downloads.locator(`a[href="${DOWNLOAD_BASE}/${file}"]`),
                `${file} should be linked exactly once in the download block`,
            ).toHaveCount(1);
        }
    });

    test('the download block names the release it is offering', async ({ page }) => {
        await expect(page.locator('#downloads')).toContainText(RELEASE_VERSION);
    });

    // The complaint this change answers: the hero's call to action scrolled the
    // page instead of doing anything.
    test('the hero acts rather than scrolls', async ({ page }) => {
        const primary = page.locator('.hero-actions .btn-download');
        await expect(primary).toHaveCount(1);
        const href = await primary.getAttribute('href');
        expect(href, 'the primary action must not be a same-page anchor').not.toMatch(/^#/);
        expect(href).toMatch(/\/releases\/(?:download|latest)/);
    });

    // The other half of the complaint: four bordered cells that looked like a
    // segmented control and were inert spans.
    test('nothing styled as a control is inert', async ({ page }) => {
        await expect(page.locator('.hero-platforms')).toHaveCount(0);

        const controls = page.locator(
            '.btn-primary, .btn-secondary, .btn-download, .btn-download-row',
        );
        const count = await controls.count();
        expect(count).toBeGreaterThan(0);
        for (let i = 0; i < count; i++) {
            await expect(controls.nth(i)).toHaveAttribute('href', /.+/);
        }
    });

    test('a download and the npm command are above the fold at 1440x900', async ({ page }) => {
        await page.setViewportSize({ width: 1440, height: 900 });
        await page.goto('/');
        for (const selector of ['.hero-actions .btn-download', '.hero-npm']) {
            const box = await page.locator(selector).boundingBox();
            expect(box, `${selector} should be laid out`).not.toBeNull();
            expect(
                box!.y + box!.height,
                `${selector} must be fully visible without scrolling`,
            ).toBeLessThanOrEqual(900);
        }
    });

    test('the releases page stays reachable', async ({ page }) => {
        await expect(page.locator(`a[href="${RELEASES_LATEST}"]`).first()).toBeVisible();
    });

    test('every supported platform and artifact is named', async ({ page }) => {
        const downloads = page.locator('#downloads');
        await expect(downloads).toContainText('macOS');
        await expect(downloads).toContainText('11.0+');
        await expect(downloads).toContainText('Windows');
        await expect(downloads).toContainText('NSIS');
        await expect(downloads).toContainText('portable');
        await expect(downloads).toContainText('Linux');
        await expect(downloads).toContainText('.deb');
        await expect(downloads).toContainText('.AppImage');
        await expect(downloads).toContainText('Terminal UI');
        await expect(downloads).toContainText('Local web server');
        await expect(downloads).toContainText('arm64');
    });

    test('the unsigned caveat is stated and links troubleshooting', async ({ page }) => {
        const downloads = page.locator('#downloads');
        await expect(downloads).toContainText('unsigned');
        await expect(downloads.locator('a[href="/docs/troubleshooting"]')).toHaveCount(1);
    });

    test('the v0.x early-development line is present', async ({ page }) => {
        await expect(page.locator('body')).toContainText('early, active development', {
            ignoreCase: true,
        });
    });

    test('the unauthenticated --bind warning accompanies the server download', async ({ page }) => {
        await expect(page.locator('#downloads')).toContainText('unauthenticated');
    });

    // The npm channel is the only route with no archive to extract and no
    // quarantine step, and it shipped several releases before this page said so.
    test('the npm channel is offered alongside the downloads', async ({ page }) => {
        const downloads = page.locator('#downloads');
        await expect(downloads).toContainText('npx @avantmedia/specforge');
        await expect(downloads).toContainText('no quarantine flag');
    });
});

// The page names a version deliberately — that is the point of this block. An
// install *command* is different: `npx` resolves the newest release itself, and
// a pinned one would go stale in a way the release guard cannot see.
test.describe('Install commands stay unpinned', () => {
    for (const route of ROUTES) {
        test(`${route} pins no version in an install command`, async ({ page }) => {
            await page.goto(route);
            const body = await page.locator('body').innerText();
            expect(body).not.toMatch(/@avantmedia\/specforge@\d/);
            expect(body).not.toMatch(/npm\s+install\s+(?:-g\s+)?@avantmedia\/specforge@/);
        });
    }
});

test.describe('Without JavaScript', () => {
    test.use({ javaScriptEnabled: false });

    // Detection runs in an effect, so the server-rendered control is what ships
    // to a visitor with scripting unavailable. It must still work.
    test('the neutral control resolves and every asset stays reachable', async ({ page }) => {
        await page.goto('/');
        const primary = page.locator('.hero-actions .btn-download');
        await expect(primary).toHaveAttribute('href', RELEASES_LATEST);
        await expect(primary).toHaveAttribute('data-platform', 'unknown');
        await expect(primary).toContainText('Download SpecForge');

        const downloads = page.locator('#downloads');
        for (const file of ASSET_FILES) {
            await expect(downloads.locator(`a[href="${DOWNLOAD_BASE}/${file}"]`)).toHaveCount(1);
        }
    });
});

test.describe('The npm channel', () => {
    // Unscoped, `specforge` on the public registry is an unrelated project, so a
    // dropped scope would not be a typo — it would send readers to someone
    // else's package. Assert the shape that must never render.
    for (const path of ['/', '/docs', '/docs/web-ui']) {
        test(`${path} never names the package without its scope`, async ({ page }) => {
            await page.goto(path);
            const body = await page.locator('body').innerText();
            expect(body).not.toMatch(/\bnpx\s+specforge\b/);
            expect(body).not.toMatch(/\bnpm\s+install\s+(?:-g\s+)?specforge\b/);
        });
    }

    test('the web UI docs carry both install commands', async ({ page }) => {
        await page.goto('/docs/web-ui');
        await expect(page.locator('h2#install')).toBeVisible();
        const body = page.locator('body');
        await expect(body).toContainText('npx @avantmedia/specforge');
        await expect(body).toContainText('npm install -g @avantmedia/specforge');
    });

    // The quarantine dance is the friction npm removes; the page that documents
    // that dance should say so, and the link is what makes it actionable.
    test('the quarantine section points at the route that avoids it', async ({ page }) => {
        await page.goto('/docs/troubleshooting');
        await expect(page.locator('a[href="/docs/web-ui#install"]')).toHaveCount(1);
    });
});
