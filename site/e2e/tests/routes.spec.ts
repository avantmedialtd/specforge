import { expect, test } from '@playwright/test';

// The ten public routes, in nav order. The 404 document is deliberately not
// here: it is not a route, and its own expectations live at the bottom.
const ROUTES = [
    { path: '/', heading: 'Spec-driven work, in full view.' },
    { path: '/changelog', heading: 'Changelog' },
    { path: '/docs', heading: 'Getting started' },
    { path: '/docs/workspaces', heading: 'Workspaces' },
    { path: '/docs/dashboard', heading: 'Dashboard' },
    { path: '/docs/commit-graph', heading: 'Reading the commit graph' },
    { path: '/docs/terminal-ui', heading: 'Terminal UI' },
    { path: '/docs/web-ui', heading: 'Web UI & remote access' },
    { path: '/docs/settings', heading: 'Settings' },
    { path: '/docs/troubleshooting', heading: 'Troubleshooting' },
];

test.describe('SpecForge routes', () => {
    for (const route of ROUTES) {
        test(`${route.path} serves 200 and renders its H1`, async ({ page }) => {
            const response = await page.goto(route.path);
            expect(response?.status(), `${route.path} should be 200`).toBe(200);
            await expect(page.locator('h1')).toHaveText(route.heading);
        });
    }

    test('every docs page is reachable from the docs sidebar', async ({ page }) => {
        await page.goto('/docs');
        const nav = page.locator('nav[aria-label="Documentation"]');
        for (const route of ROUTES.filter(r => r.path.startsWith('/docs'))) {
            await expect(nav.locator(`a[href="${route.path}"]`)).toHaveCount(1);
        }
    });

    test('the current docs page is marked aria-current in the sidebar', async ({ page }) => {
        await page.goto('/docs/settings');
        const current = page.locator('nav[aria-label="Documentation"] a[aria-current="page"]');
        await expect(current).toHaveCount(1);
        await expect(current).toHaveAttribute('href', '/docs/settings');
    });

    test('the header links to docs, GitHub, and the downloads block', async ({ page }) => {
        await page.goto('/');
        const nav = page.locator('nav[aria-label="Primary"]');
        await expect(nav.locator('a[href="/docs"]')).toHaveCount(1);
        await expect(
            nav.locator('a[href="https://github.com/avantmedialtd/specforge"]'),
        ).toHaveCount(1);
        await expect(nav.locator('a[href="/#downloads"]')).toHaveCount(1);
    });

    // The repository link renders as GitHub's mark rather than the word, so its
    // accessible name is now the only thing naming it. Asserting the href alone —
    // which is all this file did — passes just as happily on a nameless link.
    test('the header GitHub link is announced by name', async ({ page }) => {
        await page.goto('/');
        const nav = page.locator('nav[aria-label="Primary"]');
        await expect(nav.getByRole('link', { name: 'GitHub', exact: true })).toHaveAttribute(
            'href',
            'https://github.com/avantmedialtd/specforge',
        );
    });

    test('the footer links every docs page and the studio', async ({ page }) => {
        await page.goto('/');
        const footer = page.locator('nav[aria-label="Footer"]');
        for (const route of ROUTES.filter(r => r.path.startsWith('/docs'))) {
            await expect(footer.locator(`a[href="${route.path}"]`)).toHaveCount(1);
        }
        await expect(
            page.locator('footer a[href="https://www.avantmedia.uk"]').first(),
        ).toBeVisible();
    });

    test('an unknown path renders the 404 document', async ({ page }) => {
        const response = await page.goto('/does-not-exist');
        expect(response?.status()).toBe(404);
        await expect(page.locator('h1')).toHaveText('Page not found');
    });

    test('every page exposes a skip link as the first tab stop', async ({ page }) => {
        await page.goto('/');
        const skip = page.locator('a.skip-link');
        await expect(skip).toHaveAttribute('href', '#main');
        await page.keyboard.press('Tab');
        await expect(skip).toBeFocused();
    });
});
