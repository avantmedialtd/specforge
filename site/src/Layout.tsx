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

export function Layout({ currentPath, children }: { currentPath: string; children: ReactNode }) {
    const inDocs = currentPath === '/docs' || currentPath.startsWith('/docs/');

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
                        <a
                            href={REPO_URL}
                            className="text-[var(--text-muted)] no-underline hover:text-[var(--text)]"
                        >
                            GitHub
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
