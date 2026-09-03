/**
 * Typed shape of every page's `documentProps` — the single per-page metadata
 * object that drives the rendered `<head>` (via `documentMeta.ts`) and the
 * build-time discovery artifacts (sitemap / search index).
 *
 * This replaces the Next.js Metadata API: instead of `export const metadata`
 * and `buildMetadata`, each page exports a colocated `documentProps`.
 */

/** A 1200×630 social-share image (Open Graph + Twitter). */
export interface OgImage {
    url: string;
    width: number;
    height: number;
    alt: string;
}

export interface SiteDocumentProps {
    /**
     * Page-specific title portion. The renderer appends the brand suffix
     * (" | Avant Media") unless {@link titleAbsolute} is set.
     */
    title: string;
    /** When true, `title` is the full `<title>` verbatim (e.g. the home page). */
    titleAbsolute?: boolean;
    /** 120–160 character meta description. */
    description: string;
    /** Canonical route path, e.g. `/services` or `/insights/ai-slop`. */
    path: string;
    /** Open Graph type; article pages pass `'article'`. */
    ogType?: 'website' | 'article';
    /** Page-specific OG image; falls back to the site default in the renderer. */
    image?: OgImage;
    /**
     * Reciprocal hreflang alternates (`en-GB` / `hu-HU` / `x-default`). When
     * omitted, the renderer derives them from `path` via the app's hreflang fn.
     */
    languages?: Record<string, string>;
    /** ISO `YYYY-MM-DD` publish date — required for article pages. */
    date?: string;
    /**
     * ISO `YYYY-MM-DD` date this page's content last changed — the `<lastmod>`
     * source for every page in the sitemap. Falls back to {@link date} when
     * absent, so dated articles need only one of the two, but a page carrying
     * neither cannot be emitted: the discovery plugin fails the build rather
     * than substituting a placeholder. Authored and committed, never derived
     * from git or mtime (the Docker build context excludes `.git`).
     */
    modified?: string;
    /** Registry/listing category, e.g. `'Insights'` or `'Portfolio'`. */
    category?: string;
    /** When true, emit `<meta name="robots" content="noindex,follow">`. */
    noindex?: boolean;
    /** When false, exclude this route from the generated sitemap. Default true. */
    inSitemap?: boolean;
    /**
     * Human-facing on-site search membership (Avant Search) — a separate axis
     * from {@link noindex}, which governs Google.
     *
     * Defaults to the inverse of `noindex`, because the usual reason to hide a
     * page from search engines (a legal page for a third-party app, an error
     * page) is also a reason to keep it out of the site's own search. Set it
     * explicitly to decouple them: radar blips withheld from Google purely for
     * crawl budget pass `true` so visitors can still find them. `draft`
     * excludes a page from both regardless.
     */
    inSearchIndex?: boolean;
    /**
     * When true, this page is an unpublished draft. The single source of truth
     * for draft status: a default `vike build` omits the route from the
     * prerender set (no HTML in `dist/client`, 404 in production), and the page
     * is excluded from every discovery artifact (sitemap / feed / search index)
     * and the post registry in every build mode. `draft` also implies
     * `noindex`. An opt-in `INCLUDE_DRAFTS=1` build prerenders drafts (still
     * excluded from discovery) for a deployable preview. Drafts always render
     * under `vike dev`.
     */
    draft?: boolean;
}

/** A page enumerated at build time from its colocated `meta.ts`. */
export interface BuildPage {
    /** Route path: `/` for the home page, else the page's path. */
    url: string;
    title: string;
    description: string;
    date?: string;
    modified?: string;
    category?: string;
    noindex?: boolean;
    inSitemap: boolean;
    /** On-site search membership; independent of {@link BuildPage.noindex}. */
    inSearchIndex: boolean;
    /** When true, an unpublished draft — excluded from prod prerender + all discovery. */
    draft?: boolean;
}

/** Does a `documentProps` describe a dated article (registry/feed member)? */
export function isArticle(meta: SiteDocumentProps): boolean {
    return typeof meta.date === 'string' && meta.date.length > 0;
}
