import { defineConfig, devices } from '@playwright/test';

// The site is a static prerender, so the suite runs against `vike preview`
// serving `dist/` — the same sirv-based directory-index behaviour the production
// CloudFront function reproduces (`/docs/workspaces` -> `/docs/workspaces/index.html`
// with no redirect, and a 404 body from `404.html`).
//
// This config must live at `site/`, not `site/e2e/`: Playwright resolves its
// config from the working directory, and `webServer.cwd` defaults to the config
// file's own directory — which is what makes `bun run preview` find `dist/`.
const PORT = 4173;
const BASE_URL = `http://127.0.0.1:${PORT}`;

export default defineConfig({
    testDir: './e2e/tests',
    outputDir: './e2e/test-results',
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 2 : 0,
    reporter: process.env.CI ? 'github' : 'list',

    use: {
        baseURL: BASE_URL,
        trace: 'on-first-retry',
    },

    // Chromium only, at two viewports. The visual-regression suite that needed
    // pixel-stable WebKit baselines did not come across in the move, so there is
    // nothing here that a second engine would catch.
    projects: [
        {
            name: 'desktop',
            use: { ...devices['Desktop Chrome'], viewport: { width: 1920, height: 1080 } },
        },
        {
            name: 'mobile',
            use: { ...devices['Desktop Chrome'], viewport: { width: 430, height: 932 } },
        },
    ],

    webServer: {
        command: 'bun run preview',
        url: BASE_URL,
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
    },
});
