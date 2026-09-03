import { expect, test } from '@playwright/test';

// The site sets no cookies and loads no analytics, which is also why it carries
// no cookie banner and no privacy page.
const ROUTES = ['/', '/docs', '/docs/settings', '/docs/troubleshooting'];

const ANALYTICS_HOSTS = [
    'googletagmanager.com',
    'google-analytics.com',
    'plausible.io',
    'cdn.plausible.io',
    'usefathom.com',
    'simpleanalytics.com',
    'cabin.dev',
];

test.describe('Cookies and analytics posture', () => {
    for (const route of ROUTES) {
        test(`sets no cookies on ${route}`, async ({ page, context }) => {
            await page.goto(route);
            await page.waitForLoadState('networkidle');

            const cookies = await context.cookies();
            expect(
                cookies,
                `Expected no cookies, got: ${cookies.map(c => c.name).join(', ')}`,
            ).toHaveLength(0);
        });
    }

    test('loads no analytics or tracking script', async ({ page }) => {
        const offenders: string[] = [];
        page.on('request', request => {
            const url = request.url();
            if (ANALYTICS_HOSTS.some(host => url.includes(host))) {
                offenders.push(url);
            }
        });

        for (const route of ROUTES) {
            await page.goto(route);
            await page.waitForLoadState('networkidle');
        }

        expect(offenders, `Analytics requests: ${offenders.join(', ')}`).toHaveLength(0);
    });

    test('makes no third-party request at all', async ({ page, baseURL }) => {
        // Derived from the fixture rather than hardcoded: the site is served from
        // a different host in CI, in local preview, and in production.
        const selfHost = new URL(baseURL!).hostname;
        const external: string[] = [];
        page.on('request', request => {
            const url = new URL(request.url());
            if (url.hostname !== selfHost && url.protocol !== 'data:') {
                external.push(request.url());
            }
        });

        await page.goto('/');
        await page.waitForLoadState('networkidle');

        expect(external, `Third-party requests: ${external.join(', ')}`).toHaveLength(0);
    });
});
