import type { ReactNode } from 'react';
import { DOCS_NAV, REPO_URL, STUDIO_URL } from './site-config';

/**
 * The SpecForge site chrome, rendered into `#root` on both server and client so
 * it hydrates with the page.
 *
 * The site's own composition. It shares its SEO head tags with the studio sites
 * the vendored `site-kit/` came from, and nothing visual — the product should
 * not look like the studio that happens to build it.
 */
/**
 * The product lockup: SpecForge's own app icon beside the wordmark.
 *
 * 32px is the size, not a smaller one, because the product's `public/favicon.svg`
 * records that below about 32px the illustration's frame, hammer, sparks and
 * task-list detail collapse into an indistinct blob. That is measured, not
 * cautious: at 20px and at 24px the tile renders as a smudge inside a gold
 * picture frame on a 1x display. 32px is the first size where the three elements
 * resolve, so it is the smallest size at which this artwork is the right artwork.
 * It costs nothing in layout — the Download button already sets a 36px flex line
 * inside a 60px row (61px header with its border), so the 32px tile fits inside
 * the height the header already had.
 *
 * A square mark is also the only kind that aligns for free. An earlier revision
 * used the anvil glyph, whose left horn tapers to a point: its geometric bounding
 * box was correct, but the box's left edge carried almost no ink, so the mark
 * read as sitting ~8px inboard of the page's left margin even though the box was
 * flush with it. A rounded square has no such gap between its geometric and its
 * optical edge, so `items-center` and the shared left margin both land without
 * hand-tuned offsets.
 *
 * `srcset` rather than one asset: at 32px CSS a 2x display wants 64px and a 3x
 * display 96px. The descriptors are densities, not sizes — the 64px file *is* the
 * 2x candidate for a 32px box, so it must be declared `2x`. Writing it `1x` (as an
 * earlier revision did) tells the browser it is only adequate at 1x and sends every
 * Retina display to the 180px file instead: 59,643 bytes where 8,710 would do.
 *
 * `leading-none` is load-bearing. `items-center` centres the tile on the flex
 * line, and with the inherited 1.6 line-height that line is 25.6px of mostly
 * leading — so the mark centred on the leading rather than on the wordmark,
 * landing below the cap-height centre and overhanging the baseline. Collapsing
 * the line to the wordmark's own box makes the two centres coincide.
 */
function SpecForgeMark() {
    // The type bump and the wider gap are `sm:`-only. At the base size the lockup
    // is ~14px wider than the bare tile was, which is enough to push the row past
    // a 390px phone and wrap the nav onto a second line — doubling the chrome on
    // exactly the width most phones report. The well itself stays at every size;
    // it is the treatment.
    //
    // 20px is measured, not chosen for looks. Centring the wordmark against the
    // 32px tile lands on a half pixel at some type sizes and exactly at others,
    // because the text's content box is a rounded multiple of the font size:
    // 15px→0, 16px→0, 17px→0.5, 18px→0.5, 19px→0, 20px→0, 21px→0, 22px→0.
    // The mock's 17px was one of the two sizes in that range that cannot line up.
    return (
        <span className="inline-flex items-center gap-2 font-semibold leading-none tracking-tight sm:gap-[11px] sm:text-xl sm:tracking-[-0.012em]">
            {/* The well. The app icon is a fully-bleeding illustrated tile, which on
                its own reads as a sticker laid on top of the chrome — most visibly
                on the light header, where a near-black square sits on white. Three
                pixels of recessed surface and a hairline ring give it something to
                sit IN, which is the whole of this treatment; the icon itself is
                untouched at the 32px its detail needs. */}
            <span className="flex shrink-0 rounded-lg bg-[var(--mark-well)] p-0.5 ring-1 ring-inset ring-[var(--border)] sm:rounded-[9px] sm:p-[3px]">
                <img
                    src="/specforge-icon-64.png"
                    srcSet="/specforge-icon-64.png 2x, /specforge-icon-180.png 3x"
                    width="32"
                    height="32"
                    alt=""
                    className="block rounded-md"
                />
            </span>
            SpecForge
        </span>
    );
}

/**
 * GitHub's mark, for the nav's one off-site link.
 *
 * The path is GitHub's own `mark-github-16` octicon, copied verbatim from
 * https://raw.githubusercontent.com/primer/octicons/main/icons/mark-github-16.svg
 * rather than retyped: at 16px a dropped subpath or a wrong fill-rule is invisible
 * in review but wrong on the page. Do not tidy it.
 *
 * `fill="currentColor"` and no colour of its own — that is the whole reason this is
 * inline SVG and not an `<img>`. The link keeps inheriting `--text-muted`, keeps its
 * `hover:text-[var(--text)]`, and is correct in both themes with one asset instead
 * of a light and a dark file.
 *
 * 16px, not the reflexive 20px. A filled logo carries far more ink per unit area
 * than the 14px word it replaces, so at an identical `--text-muted` it reads
 * *heavier* than `Docs` beside it — the opposite of the usual worry. The nav
 * comment below forbids retuning that token, so size is the only lever left for
 * optical balance, and it has to be pulled down rather than compensated for with a
 * lighter colour. Compared against `Docs` in both themes at 16, 18 and 20: at 20 the
 * mark plainly dominates the word beside it, at 18 it still reads heavier, and 16 is
 * the one that matches.
 *
 * `block` on the svg for the same reason `leading-none` is load-bearing on the
 * lockup above: an inline replaced element sits on a text baseline and drags the
 * anchor's line box with it.
 *
 * `aria-hidden` because the anchor carries the accessible name. The desktop app's
 * `src/components/icons.tsx` routes names through an SVG `<title>` with
 * `role="img"`, but that file is a 24x24 `stroke` system this filled mark could not
 * join, and it is in a package `site/` cannot import anyway — so this is a parallel
 * one-off, deliberately, and the name belongs on the link rather than on the image
 * inside it.
 */
function GitHubMark() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="currentColor"
            aria-hidden="true"
            className="block"
        >
            <path d="M6.766 11.328c-2.063-.25-3.516-1.734-3.516-3.656 0-.781.281-1.625.75-2.188-.203-.515-.172-1.609.063-2.062.625-.078 1.468.25 1.968.703.594-.187 1.219-.281 1.985-.281.765 0 1.39.094 1.953.265.484-.437 1.344-.765 1.969-.687.218.422.25 1.515.046 2.047.5.593.766 1.39.766 2.203 0 1.922-1.453 3.375-3.547 3.64.531.344.89 1.094.89 1.954v1.625c0 .468.391.734.86.547C13.781 14.359 16 11.53 16 8.03 16 3.61 12.406 0 7.984 0 3.563 0 0 3.61 0 8.031a7.88 7.88 0 0 0 5.172 7.422c.422.156.828-.125.828-.547v-1.25c-.219.094-.5.156-.75.156-1.031 0-1.64-.562-2.078-1.609-.172-.422-.36-.672-.719-.719-.187-.015-.25-.093-.25-.187 0-.188.313-.328.625-.328.453 0 .844.281 1.25.86.313.452.64.655 1.031.655s.641-.14 1-.5c.266-.265.47-.5.657-.656" />
        </svg>
    );
}

export function Layout({ currentPath, children }: { currentPath: string; children: ReactNode }) {
    const inDocs = currentPath === '/docs' || currentPath.startsWith('/docs/');
    const inChangelog = currentPath === '/changelog';

    return (
        <div className="min-h-screen flex flex-col bg-[var(--bg)] text-[var(--text)]">
            <a href="#main" className="skip-link">
                Skip to main content
            </a>

            <header className="sticky top-0 z-20 border-b border-[var(--border)] bg-[var(--surface)]/90 backdrop-blur">
                {/* `flex-wrap` is load-bearing, not defensive: brand + three nav
                    items have an intrinsic width of 373px, so without it every
                    page scrolled sideways on a 360px-wide phone. The footer nav
                    below already wraps for the same reason. */}
                {/* `flex-wrap` is load-bearing, not defensive: brand + three nav items
                    have an intrinsic width the row cannot always hold, so without it
                    every page scrolled sideways on a narrow phone. The base column gap
                    is 12px rather than 16px to buy that row back ~4px: the seated mark
                    is 6px wider than the bare tile, which was enough to push a 390px
                    phone — the width most report — from fitting on one line to wrapping
                    into a doubled-height header. Measured slack at 390px is ~3px, the
                    same as before the mark was seated. */}
                <div className="site-shell flex flex-wrap items-center gap-x-3 gap-y-2 py-4 sm:gap-x-6">
                    {/* `inline-flex` is load-bearing, not cosmetic: it is what lets the
                        row's `items-center` reach the mark at all. Without it the anchor
                        blockifies to `block` but still runs an inline formatting context,
                        so it carries a strut from its own inherited 16px/1.6 — 25.594px,
                        split 18.000 above the baseline and 7.594 below. The lockup inside
                        is an `inline-flex` whose own `align-items: center` leaves no
                        baseline-aligned child, so its baseline is synthesised at its
                        bottom margin edge: all 32px hangs above the baseline with nothing
                        balancing that 7.594px descent. The line box became
                        12.000..51.594 = 39.594px around a 32px mark, and `items-center`
                        faithfully centred *that*, leaving the lockup half the descent —
                        3.797px — above the centre the nav and Download button share.
                        Removing the line box is the fix; `leading-none` here is not, and
                        was measured leaving 1px. `inline-flex` over `flex` because a flex
                        item blockifies either to the same thing, and this spelling does
                        not stretch if the lockup is ever used outside a flex row. */}
                    <a
                        href="/"
                        className="inline-flex text-[var(--text)] no-underline hover:text-[var(--accent)]"
                    >
                        <SpecForgeMark />
                    </a>
                    {/* `font-medium` holds the nav's weight against the seated mark,
                        which carries more visual weight than the bare tile did. The
                        colour stays `--text-muted`, whose contrast ratios are the
                        documented ones — brightening it here would silently retune a
                        token the rest of the site relies on. */}
                    <nav
                        aria-label="Primary"
                        className="ml-auto flex items-center gap-4 text-sm font-medium sm:gap-5"
                    >
                        {/* A peer of `Docs` rather than a docs page: its audience
                            includes visitors who are not reading documentation,
                            and its content is authored by `/release` rather than
                            by this site. Putting it in `DOCS_NAV` instead would
                            file the release history as reference material and
                            bury it behind the docs shell.

                            `max-sm:hidden` is load-bearing, not a preference.
                            The row's intrinsic width was measured at 373px for
                            brand + three items, which already only just clears a
                            360px phone; a fourth item pushes it past 320px and
                            wraps the nav onto a second line. That wrap is not
                            merely untidy — it grows the sticky header to 113px,
                            past the 88px `--anchor-offset` that every deep link
                            relies on, and it moves the nav's centre away from the
                            brand's and the button's. Three tests catch each of
                            those. Below `sm` the footer carries this link
                            instead, so the page stays reachable at every width. */}
                        <a
                            href="/changelog"
                            aria-current={inChangelog ? 'page' : undefined}
                            className={
                                inChangelog
                                    ? 'text-[var(--accent)] no-underline max-sm:hidden'
                                    : 'text-[var(--text-muted)] no-underline hover:text-[var(--text)] max-sm:hidden'
                            }
                        >
                            Changelog
                        </a>
                        <a
                            href="/docs"
                            aria-current={inDocs ? 'page' : undefined}
                            className={
                                inDocs
                                    ? 'text-[var(--accent)] no-underline'
                                    : 'text-[var(--text-muted)] no-underline hover:text-[var(--text)]'
                            }
                        >
                            Docs
                        </a>
                        {/* The nav's only off-site link, and until now nothing said
                            so — "GitHub" sat here as a peer of "Docs", a fourth
                            destination in the site's own IA rather than a departure
                            from it. GitHub's mark says it without spending a label.

                            `aria-label` is what keeps this honest: the computed
                            accessible name is still exactly "GitHub", so the link is
                            announced as it always was and only its rendering changed.
                            The mark inside is `aria-hidden`, so it is announced once
                            rather than twice. `routes.spec.ts` asserts that name — it
                            previously asserted the href alone, which an icon-only link
                            could satisfy while being nameless.

                            `inline-flex` is the same fix, for the same reason, as the
                            one documented at length on the brand anchor above: a bare
                            anchor here runs an inline formatting context and inherits a
                            25.594px strut from the row's 16px/1.6, so `items-center`
                            centres that line box rather than the 16px mark. Removing
                            the line box is what makes the mark share a centre with
                            `Docs` and the Download button.

                            `p-2 -m-2` is not spacing. WCAG 2.5.8 wants a 24x24 target
                            and a bare 16px mark is under it, but plain padding would
                            widen the flex line and invalidate every gap this header's
                            comments have measured to the pixel. The negative margin
                            offsets the padding exactly, so the target is 16+2*8 = 32px
                            while the layout contribution stays 16+2*8-2*8 = 16px — the
                            mark's own width. Clearance to the neighbouring text box is
                            gap minus padding: 8px at the base `gap-4`, 12px from `sm:`
                            up, so the enlarged targets never collide. */}
                        <a
                            href={REPO_URL}
                            aria-label="GitHub"
                            className="inline-flex -m-2 p-2 text-[var(--text-muted)] no-underline hover:text-[var(--text)]"
                        >
                            <GitHubMark />
                        </a>
                        <a href="/#downloads" className="btn-primary">
                            Download
                        </a>
                    </nav>
                </div>
            </header>

            <main id="main" tabIndex={-1} className="w-full flex-1 focus:outline-none">
                {children}
            </main>

            <footer className="border-t border-[var(--border)] bg-[var(--surface)]">
                <div className="site-shell py-8 text-sm text-[var(--text-muted)]">
                    <nav aria-label="Footer" className="mb-5 flex flex-wrap gap-x-5 gap-y-2">
                        {/* Not a `DOCS_NAV` entry — the changelog is not
                            documentation, and that array drives the docs sidebar
                            too. Listed here so the page stays reachable at the
                            widths where the header drops its link. */}
                        <a
                            href="/changelog"
                            className="text-[var(--text-muted)] no-underline hover:text-[var(--text)]"
                        >
                            Changelog
                        </a>
                        {DOCS_NAV.map(item => (
                            <a
                                key={item.href}
                                href={item.href}
                                className="text-[var(--text-muted)] no-underline hover:text-[var(--text)]"
                            >
                                {item.label}
                            </a>
                        ))}
                    </nav>
                    <p className="m-0">
                        SpecForge is built by{' '}
                        <a href={STUDIO_URL} className="text-[var(--accent)]">
                            Avant Media
                        </a>
                        . MIT licensed — source on{' '}
                        <a href={REPO_URL} className="text-[var(--accent)]">
                            GitHub
                        </a>
                        .
                    </p>
                </div>
            </footer>
        </div>
    );
}
