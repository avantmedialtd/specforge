import { expect, test } from '@playwright/test';
import { RELEASE_VERSION } from '../../src/site-config';

/**
 * Structure only — never wording.
 *
 * This page's content is release-note prose authored by `/release` and it
 * changes on every release. A test that asserted any of it would have to be
 * edited each time a release ships, which is exactly the coupling the rest of
 * this suite avoids by deriving every version-bearing expectation from
 * `site-config.ts`.
 *
 * What is worth guarding is that the page is not silently empty or broken: the
 * build-time pipeline that reads `releases/`, cuts each note at its Downloads
 * footer and converts it to HTML has several ways to produce a page that
 * responds 200 and says nothing. The assertions below are the smallest set that
 * catches that while staying indifferent to what the current release says.
 */
test.describe('Changelog', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/changelog');
    });

    test('names the release the site advertises', async ({ page }) => {
        // Read from the same constant the page reads. Like the downloads suite,
        // this deliberately does not prove the version is current — that is
        // `release.yml`'s `check-site-version` job.
        await expect(page.locator('h1')).toHaveText('Changelog');
        // Scoped to the article: the page also carries an "Earlier releases"
        // h2, and an unscoped level-2 role query matches both.
        await expect(page.locator('article.prose-notes h2')).toContainText(
            `v${RELEASE_VERSION}`,
        );
    });

    test('renders the current release rather than an empty shell', async ({ page }) => {
        const article = page.locator('article.prose-notes');
        await expect(article).toHaveCount(1);

        // The notes' own sections are demoted to h3. At least one must survive
        // the cut, or the pipeline produced a heading and nothing beneath it.
        expect(await article.locator('h3').count()).toBeGreaterThan(0);
        expect(await article.locator('li').count()).toBeGreaterThan(0);
    });

    test('publishes no Downloads footer', async ({ page }) => {
        // The footer duplicates the site's own download block and names
        // version-pinned artefacts that go stale on an older release's entry.
        // Asserted by its own markers, not by artefact extensions: a changelog
        // bullet may legitimately name one — v0.0.2's whole entry is about the
        // platform downloads that were missing from v0.0.1.
        const body = await page.locator('body').innerText();
        expect(body).not.toContain('Full Changelog');
        expect(body).not.toContain('com.apple.quarantine');
        await expect(
            page.locator('article.prose-notes').getByRole('heading', { name: 'Downloads' }),
        ).toHaveCount(0);
    });

    test('lists earlier releases and links each one', async ({ page }) => {
        const earlier = page.locator('section a[href*="/releases/tag/"]');
        // A floor, not a count: the exact number grows with every release, and
        // an assertion that tracked it would need editing each time.
        expect(await earlier.count()).toBeGreaterThan(5);
    });

    test('every generated heading id is unique', async ({ page }) => {
        // The notes reuse five section names across every release, so an
        // un-namespaced slugger would emit repeated ids and break deep links.
        const ids = await page.locator('article.prose-notes [id]').evaluateAll(nodes =>
            nodes.map(n => n.id),
        );
        expect(ids.length).toBeGreaterThan(0);
        expect(new Set(ids).size).toBe(ids.length);
    });

    test('is reachable from every page, at every width', async ({ page }) => {
        await page.goto('/');
        const header = page.locator('nav[aria-label="Primary"] a[href="/changelog"]');
        const footer = page.locator('nav[aria-label="Footer"] a[href="/changelog"]');

        // Wide enough for the header link. A fourth nav item does not fit a
        // narrow phone — see the comment on the link in `Layout.tsx` — so the
        // header drops it below `sm` and the footer is what keeps the page
        // reachable there. Assert visibility rather than presence, because the
        // link stays in the DOM either way.
        await page.setViewportSize({ width: 1280, height: 900 });
        await expect(header).toBeVisible();

        await page.setViewportSize({ width: 360, height: 900 });
        await expect(header).toBeHidden();
        await expect(footer).toBeVisible();
    });
});
