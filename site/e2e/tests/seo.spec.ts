import { expect, test } from '@playwright/test';

const ORIGIN = 'https://specforge.avantmedia.uk';

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

test.describe('SpecForge SEO', () => {
    test('every route canonicalises to its own URL on the SpecForge origin', async ({ page }) => {
        for (const route of ROUTES) {
            await page.goto(route);
            const canonical = await page.locator('link[rel="canonical"]').getAttribute('href');
            const expected = route === '/' ? ORIGIN : `${ORIGIN}${route}`;
            expect(canonical, `${route} canonical`).toBe(expected);
        }
    });

    test('no page references a studio domain in its head', async ({ page }) => {
        for (const route of ROUTES) {
            await page.goto(route);
            const head = await page.locator('head').innerHTML();
            // Match the studio hosts specifically. A substring like
            // 'avantmedia.uk/' would also match this site's own
            // specforge.avantmedia.uk canonical, which is the correct value.
            expect(head, `${route} head`).not.toContain('www.avantmedia.uk');
            expect(head, `${route} head`).not.toContain('//avantmedia.uk');
            expect(head, `${route} head`).not.toContain('www.avantmedia.hu');
            expect(head, `${route} head`).not.toContain('avantmedia-logo.svg');
        }
    });

    // This site has no counterpart in another locale, so a reciprocal alternate
    // could only point at itself.
    test('no hreflang or og:locale:alternate is emitted', async ({ page }) => {
        await page.goto('/');
        await expect(page.locator('link[rel="alternate"][hreflang]')).toHaveCount(0);
        await expect(page.locator('meta[property="og:locale:alternate"]')).toHaveCount(0);
    });

    test('the OG image is a SpecForge asset at 1200x630', async ({ page }) => {
        await page.goto('/');
        await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
            'content',
            `${ORIGIN}/og-specforge.png`,
        );
        await expect(page.locator('meta[property="og:image:width"]')).toHaveAttribute(
            'content',
            '1200',
        );
        await expect(page.locator('meta[property="og:image:height"]')).toHaveAttribute(
            'content',
            '630',
        );
    });

    test('the OG image actually resolves', async ({ request }) => {
        const response = await request.get('/og-specforge.png');
        expect(response.status()).toBe(200);
    });

    test('every route is indexable', async ({ page }) => {
        for (const route of ROUTES) {
            await page.goto(route);
            await expect(page.locator('meta[name="robots"]')).toHaveAttribute(
                'content',
                'index,follow',
            );
        }
    });

    test('the 404 document is noindex', async ({ page }) => {
        await page.goto('/does-not-exist');
        await expect(page.locator('meta[name="robots"]')).toHaveAttribute(
            'content',
            'noindex,follow',
        );
    });

    test('the sitemap lists all nine routes and nothing else', async ({ request }) => {
        const response = await request.get('/sitemap.xml');
        expect(response.status()).toBe(200);
        const xml = await response.text();

        const locs = [...xml.matchAll(/<loc>([^<]*)<\/loc>/g)].map(m => m[1]);
        const expected = ROUTES.map(r => (r === '/' ? `${ORIGIN}/` : `${ORIGIN}${r}`));
        expect(locs.sort()).toEqual(expected.sort());

        // Every entry carries a lastmod, even though this site's all share one date.
        const lastmods = [...xml.matchAll(/<lastmod>([^<]*)<\/lastmod>/g)].map(m => m[1]);
        expect(lastmods).toHaveLength(locs.length);
        for (const value of lastmods) {
            expect(value).toMatch(/^\d{4}-\d{2}-\d{2}$/);
        }
    });

    test('robots.txt advertises the sitemap and no feed', async ({ request }) => {
        const response = await request.get('/robots.txt');
        expect(response.status()).toBe(200);
        const body = await response.text();
        expect(body).toContain(`Sitemap: ${ORIGIN}/sitemap.xml`);
        expect(body).not.toContain('feed');
    });

    // A product site with no dated articles has no feed to publish; an item-less
    // channel would be noise.
    test('no feed.xml is published', async ({ request }) => {
        const response = await request.get('/feed.xml');
        expect(response.status()).toBe(404);
    });

    test('the page declares English, not a studio locale pair', async ({ page }) => {
        await page.goto('/');
        await expect(page.locator('html')).toHaveAttribute('lang', 'en');
        await expect(page.locator('meta[property="og:site_name"]')).toHaveAttribute(
            'content',
            'SpecForge',
        );
    });
});
