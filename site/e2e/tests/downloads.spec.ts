import { expect, test } from '@playwright/test';

const RELEASES_LATEST = 'https://github.com/avantmedialtd/specforge/releases/latest';

// Every public route, so the version-string guard below covers the whole site
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

    test('the download CTA points at the latest-release page', async ({ page }) => {
        await expect(page.locator(`a[href="${RELEASES_LATEST}"]`).first()).toBeVisible();
    });

    // The whole reason downloads link `releases/latest` rather than an asset URL
    // is that the page must not go stale between releases. A version string
    // anywhere in the rendered prose means someone hardcoded one — and
    // `src/site-config.ts` advertises this guard as covering every rendered
    // page, so it walks all nine rather than only the landing page.
    test('no version number appears on any rendered page', async ({ page }) => {
        const offenders: string[] = [];
        for (const route of ROUTES) {
            await page.goto(route);
            const body = await page.locator('body').innerText();
            // Deliberately NOT a bare `\d+\.\d+\.\d+`: the pages legitimately
            // print the loopback address 127.0.0.1 for specforge-serve. Match the
            // two shapes a real version takes instead — a `v`-prefixed tag, and
            // the `_<version>_` segment in a release asset filename.
            const versionLike = [
                ...body.matchAll(/\bv\d+\.\d+\.\d+\b/g),
                ...body.matchAll(/_\d+\.\d+\.\d+_/g),
            ].map(m => m[0]);
            offenders.push(...versionLike.map(v => `${route}: ${v}`));
        }
        expect(
            offenders,
            `Found version strings: ${offenders.join(', ')}. Link releases/latest instead.`,
        ).toEqual([]);
    });

    test('no download link targets a versioned release asset', async ({ page }) => {
        const hrefs = await page
            .locator('a[href*="/releases/"]')
            .evaluateAll(links => links.map(a => a.getAttribute('href') ?? ''));
        expect(hrefs.length).toBeGreaterThan(0);
        for (const href of hrefs) {
            expect(href, `${href} should be the latest-release page`).toContain('/releases/latest');
            expect(href).not.toContain('/download/');
        }
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
