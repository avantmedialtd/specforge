import { expect, test } from '@playwright/test';

const ROUTES = [
    '/',
    '/changelog',
    '/docs',
    '/docs/workspaces',
    '/docs/dashboard',
    '/docs/commit-graph',
    '/docs/terminal-ui',
    '/docs/web-ui',
    '/docs/settings',
    '/docs/troubleshooting',
];

/**
 * Computed-style guards for two defects a full-page screenshot cannot catch and
 * did not catch.
 *
 * Both came from the same root cause — author CSS that was written as if it were
 * the only stylesheet — and both were invisible to the existing suite: the
 * visual baselines had them baked in as expected output, and `routes.spec.ts`
 * asserts `aria-current` as an attribute, which was correct the whole time.
 */
test.describe('Layout regressions', () => {
    // The header row is brand + three nav items with an intrinsic width of
    // 373px. Without `flex-wrap` that overflowed a 360px phone — one of the most
    // common Android widths — and scrolled every page sideways.
    for (const width of [320, 360, 375]) {
        test(`no horizontal overflow at ${width}px`, async ({ page }) => {
            await page.setViewportSize({ width, height: 900 });
            const overflowing: string[] = [];
            for (const route of ROUTES) {
                await page.goto(route);
                await page.waitForLoadState('networkidle');
                const { scrollWidth, innerWidth } = await page.evaluate(() => ({
                    scrollWidth: document.documentElement.scrollWidth,
                    innerWidth: window.innerWidth,
                }));
                // 1px of slack for sub-pixel rounding at fractional zoom.
                if (scrollWidth > innerWidth + 1) {
                    overflowing.push(`${route} (${scrollWidth} > ${innerWidth})`);
                }
            }
            expect(overflowing, `Pages scrolling sideways at ${width}px`).toEqual([]);
        });
    }

    // An un-layered `a { … }` in styles.css outranked every Tailwind utility, so
    // the docs sidebar's current page rendered identically to the other eight.
    // Assert the rendered difference, not just the attribute.
    test('the current docs page is visually distinct in the sidebar', async ({ page }) => {
        await page.goto('/docs/settings');
        const links = page.locator('nav[aria-label="Documentation"] a');
        await expect(links).toHaveCount(8);

        const styles = await links.evaluateAll(nodes =>
            nodes.map(n => {
                const cs = getComputedStyle(n);
                return {
                    current: n.getAttribute('aria-current') === 'page',
                    colour: cs.color,
                };
            }),
        );

        const current = styles.filter(s => s.current);
        const others = styles.filter(s => !s.current);
        expect(current).toHaveLength(1);
        expect(
            others.every(o => o.colour !== current[0].colour),
            `Current page renders ${current[0].colour}; siblings render the same colour, so the active state is invisible.`,
        ).toBe(true);
    });

    // `no-underline` on the nav links must actually win over the element rule.
    test('nav links are not underlined', async ({ page }) => {
        await page.goto('/docs');
        const decorations = await page
            .locator('nav[aria-label="Primary"] a, nav[aria-label="Documentation"] a')
            .evaluateAll(nodes => nodes.map(n => getComputedStyle(n).textDecorationLine));
        expect(decorations.filter(d => d.includes('underline'))).toEqual([]);
    });

    // Every DocsSection heading carries an id so it can be linked; the sticky
    // header is 61px tall, so without scroll-margin they all land underneath it.
    test('anchor targets clear the sticky header', async ({ page }) => {
        await page.goto('/docs/web-ui');
        const offset = await page
            .locator('h2[id]')
            .first()
            .evaluate(n => parseFloat(getComputedStyle(n).scrollMarginTop));
        const headerHeight = await page
            .locator('header')
            .evaluate(n => n.getBoundingClientRect().height);
        expect(offset).toBeGreaterThanOrEqual(headerHeight);
    });

    // The header's repository link is a 16px glyph, which is under WCAG 2.5.8's
    // 24x24 minimum on its own. It is padded to 32x32 and pulled back by an equal
    // negative margin, so the target conforms while the flex line still sees only
    // the mark's own width — which is what keeps every gap this header's comments
    // have measured to the pixel valid. Assert both halves: padding without the
    // offset would widen the row, and the offset without the padding would fail
    // the minimum.
    test('the header GitHub link has a conformant target that does not widen the row', async ({
        page,
    }) => {
        await page.goto('/');
        const box = await page
            .locator('nav[aria-label="Primary"] a[href="https://github.com/avantmedialtd/specforge"]')
            .evaluate(n => {
                const cs = getComputedStyle(n);
                const r = n.getBoundingClientRect();
                const svg = n.querySelector('svg')!.getBoundingClientRect();
                return {
                    width: r.width,
                    height: r.height,
                    layout: r.width + parseFloat(cs.marginLeft) + parseFloat(cs.marginRight),
                    glyph: svg.width,
                };
            });

        expect(box.width, 'activation target width').toBeGreaterThanOrEqual(24);
        expect(box.height, 'activation target height').toBeGreaterThanOrEqual(24);
        expect(
            box.layout,
            `the link contributes ${box.layout}px to the row but the glyph is only ${box.glyph}px — the negative margin no longer offsets the padding`,
        ).toBeLessThanOrEqual(box.glyph);
    });

    // The anchor carries the accessible name; the mark inside must not also carry
    // one, or the link is announced twice.
    test('the header GitHub mark is hidden from assistive technology', async ({ page }) => {
        await page.goto('/');
        const mark = page.locator(
            'nav[aria-label="Primary"] a[href="https://github.com/avantmedialtd/specforge"] svg',
        );
        await expect(mark).toHaveAttribute('aria-hidden', 'true');
        await expect(mark.locator('title')).toHaveCount(0);
    });
});
