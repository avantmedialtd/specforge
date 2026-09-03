import { expect, test } from '@playwright/test';

test.describe('SpecForge landing page', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
    });

    test('uses SpecForge’s own app icon at the size its detail survives', async ({ page }) => {
        const mark = page.locator('header a[href="/"] img');
        await expect(mark).toHaveAttribute('src', '/specforge-icon-64.png');
        // Below ~32px the illustration's frame, hammer and task list stop
        // resolving, so shrinking this is the regression to catch.
        await expect(mark).toHaveJSProperty('width', 32);
        await expect(mark).toHaveJSProperty('height', 32);
        // 3x displays need 96px, which only the 180px asset supplies.
        await expect(mark).toHaveAttribute('srcset', /specforge-icon-180\.png 3x/);
    });

    test('shares one vertical centre across every item in the header row', async ({ page }) => {
        // The row is `flex items-center`, but centring only reaches the lockup if
        // every box between the row and the mark is itself a flex container. The
        // brand <a> was an inline element: blockified as a flex item, yet its
        // contents still formed a line box, so the 32px inline-flex lockup
        // baseline-aligned inside it and picked up the strut's descender. That
        // made the anchor 39.6px tall around a 32px mark and left the brand 3.8px
        // above the centre the nav and button shared. Comparing all three centres
        // catches any future break in that chain.
        for (const width of [1440, 1100, 900, 768, 520, 390]) {
            await page.setViewportSize({ width, height: 900 });
            const c = await page.evaluate(() => {
                const mid = (sel: string) => {
                    const r = document.querySelector(sel)!.getBoundingClientRect();
                    return (r.top + r.bottom) / 2;
                };
                return {
                    brand: mid('header a[href="/"] img'),
                    nav: mid('header nav'),
                    button: mid('header .btn-primary'),
                };
            });
            const spread = Math.max(c.brand, c.nav, c.button) - Math.min(c.brand, c.nav, c.button);
            expect(spread, `header centres disagree at ${width}px`).toBeLessThan(0.5);
        }
    });

    test('centres the product mark on the wordmark rather than on its leading', async ({
        page,
    }) => {
        // Regression guard for the lockup geometry, on two counts that both once
        // failed. The mark used to be centred on a 25.6px line box that was
        // mostly leading, leaving it below the wordmark; and its viewBox used to
        // be the source file's full 32x32 square, whose padding reopened the same
        // gap. Comparing box centres catches both without depending on font
        // rasterisation.
        const m = await page
            .locator('header a[href="/"] > span')
            .first()
            .evaluate(el => {
                const icon = el.querySelector('img') as HTMLImageElement;
                // The icon is seated inside a well, so the wordmark's gap is
                // measured from the well's edge, not the icon's.
                const well = el.firstElementChild as HTMLElement;
                const textNode = [...el.childNodes].find(
                    n => n.nodeType === Node.TEXT_NODE && n.textContent?.trim(),
                )!;
                const range = document.createRange();
                range.selectNodeContents(textNode);
                const ib = icon.getBoundingClientRect();
                const wb = well.getBoundingClientRect();
                const tb = range.getBoundingClientRect();
                return {
                    mark: (ib.top + ib.bottom) / 2,
                    text: (tb.top + tb.bottom) / 2,
                    height: ib.height,
                    wellRight: wb.right,
                    // The well must actually surround the icon on every side.
                    inset: {
                        left: +(ib.left - wb.left).toFixed(2),
                        right: +(wb.right - ib.right).toFixed(2),
                        top: +(ib.top - wb.top).toFixed(2),
                        bottom: +(wb.bottom - ib.bottom).toFixed(2),
                    },
                    textLeft: tb.left,
                };
            });

        expect(Math.abs(m.mark - m.text)).toBeLessThan(0.5);
        expect(m.height).toBeCloseTo(32, 1);
        // The well is an even recess around the icon, not a one-sided offset.
        expect(m.inset.left).toBeGreaterThan(0);
        expect(m.inset.left).toBeCloseTo(m.inset.right, 1);
        expect(m.inset.top).toBeCloseTo(m.inset.bottom, 1);
        expect(m.inset.left).toBeCloseTo(m.inset.top, 1);
        // And the gap to the wordmark is the flex gap, nothing more.
        expect(m.textLeft - m.wellRight).toBeLessThan(13);
    });

    test('hangs the header mark on the same left edge as the page content', async ({ page }) => {
        // The chrome and the page body used to run on two different containers —
        // `max-w-5xl` with 20px padding for the header and footer, 1180px with
        // 24px for the landing sections — which put the mark 74px inboard of the
        // hero copy at 1440px. Both now derive from --shell-max/--shell-pad, so
        // this checks the edges agree at the widths where they diverged, and
        // across the 767px gutter step.
        for (const width of [1440, 1280, 1100, 900, 768, 520, 390]) {
            await page.setViewportSize({ width, height: 900 });
            const edges = await page.evaluate(() => {
                // The mark's box is the well, not the icon inside it — the icon is
                // deliberately inset by the well's padding, so measuring the <img>
                // would report the page's left edge as 3px further right than it is.
                const mark = document.querySelector('header a[href="/"] > span > span')!;
                const h1 = document.querySelector('h1')!;
                return {
                    mark: mark.getBoundingClientRect().left,
                    body: h1.getBoundingClientRect().left,
                };
            });
            expect(edges.mark, `header mark vs h1 at ${width}px`).toBeCloseTo(edges.body, 0);
        }
    });

    test('leads with spec-driven development and names OpenSpec as current support', async ({
        page,
    }) => {
        const headline = page.locator('h1');
        await expect(headline).toHaveText('Spec-driven work, in full view.');
        await expect(headline).not.toContainText('OpenSpec');
        await expect(page.locator('.hero-kicker')).toContainText('spec-driven development');
        await expect(page.locator('.hero-summary')).toContainText('supports OpenSpec today');

        const sourceStory = page.locator('#worktrees');
        await expect(sourceStory).toContainText('No second source of truth');
        await expect(sourceStory).toContainText('3 worktrees resolve to');
    });

    test('keeps the landing page to five focused parts', async ({ page }) => {
        await expect(page.locator('main > section')).toHaveCount(5);
        await expect(page.locator('main h2')).toHaveText([
            'Specs give work structure. SpecForge gives it a view.',
            'Follow the thinking all the way to Git.',
            'A companion, not another system of record.',
            'Open your first workspace.',
        ]);
    });

    test('shows the product workflow from navigation to Git evidence', async ({ page }) => {
        const questions = page.locator('.question-grid article');
        await expect(questions).toHaveCount(3);
        await expect(questions.nth(0)).toContainText('Find the work in context.');
        await expect(questions.nth(1)).toContainText('Read artifacts as documents.');
        await expect(questions.nth(2)).toContainText('Connect intent to evidence.');
    });

    test('states the read-only boundary without claiming SpecForge writes nothing', async ({
        page,
    }) => {
        const section = page.locator('#read-only');
        await expect(section).toContainText('does not edit a spec');
        await expect(section).toContainText('does not', { ignoreCase: true });
        await expect(section).toContainText('writes only its own app state');
    });

    test('gives an accurate npm first run', async ({ page }) => {
        const downloads = page.locator('#downloads');
        await expect(downloads).toContainText('npx @avantmedia/specforge');
        await expect(downloads).toContainText("add the workspace's host path in Settings", {
            ignoreCase: true,
        });
        await expect(downloads).not.toContainText('runs it in any workspace', {
            ignoreCase: true,
        });
    });

    test('stays within the viewport at every landing-page breakpoint', async ({ page }) => {
        for (const width of [519, 520, 767, 768, 1023, 1024]) {
            await page.setViewportSize({ width, height: 900 });
            const { scrollWidth, innerWidth } = await page.evaluate(() => ({
                scrollWidth: document.documentElement.scrollWidth,
                innerWidth: window.innerWidth,
            }));
            expect(scrollWidth, `horizontal overflow at ${width}px`).toBeLessThanOrEqual(
                innerWidth + 1,
            );
        }

        await page.setViewportSize({ width: 1023, height: 900 });
        const belowDesktopColumns = await page
            .locator('.hero-inner')
            .evaluate(element => getComputedStyle(element).gridTemplateColumns.split(' ').length);
        expect(belowDesktopColumns).toBe(1);

        await page.setViewportSize({ width: 1024, height: 900 });
        const desktopColumns = await page
            .locator('.hero-inner')
            .evaluate(element => getComputedStyle(element).gridTemplateColumns.split(' ').length);
        expect(desktopColumns).toBe(2);
    });
});
