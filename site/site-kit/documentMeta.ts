import type { OgImage, SiteDocumentProps } from './postMeta';

/**
 * The brand-neutral half of a site's render configuration — everything the
 * shared head tags need, and nothing that assumes the Avant Media chrome.
 *
 * Vendored from the studio's site-kit as the brand-neutral half only: the
 * chrome-configuration types this module was written to avoid were never
 * copied across, so nothing here reaches a shared design system.
 */
export interface DocumentMetaConfig {
    /** Production origin, e.g. `https://www.avantmedia.uk`. No trailing slash. */
    siteUrl: string;
    /** Brand suffix appended to non-absolute titles, e.g. `Avant Media`. */
    brand: string;
    /** Open Graph locale, e.g. `en_GB` or `hu_HU`. */
    ogLocale: string;
    /** Open Graph site name. */
    ogSiteName: string;
    /** `<meta name="theme-color">` value. */
    themeColor: string;
    /** Default OG image used when a page declares none. */
    defaultOgImage: OgImage;
}

export interface DocumentMetaInput {
    /** Resolved canonical route path, e.g. `/services` or `/`. */
    path: string;
    /** The page's `documentProps`, when it declares any. */
    props?: SiteDocumentProps;
    /**
     * Pre-rendered `<link rel="alternate" hreflang=…>` tags, already joined with
     * `'\n        '`. Supplied by the caller because reciprocal alternates are a
     * property of a site that HAS a counterpart; a single-locale site passes
     * nothing and emits none.
     */
    hreflangTags?: string;
    /**
     * Pre-rendered `og:locale:alternate` meta INCLUDING its leading newline and
     * indentation, or `''`. Kept in lockstep with `hreflangTags` by the caller.
     */
    ogLocaleAlternate?: string;
}

/** The brand-neutral head content for one page. */
export interface DocumentMeta {
    /** Full `<title>` text, with the brand suffix already applied. */
    title: string;
    /** Meta description text. */
    description: string;
    /** Absolute canonical URL. */
    canonical: string;
    /**
     * The head tag block: canonical, robots, theme-color, Open Graph, Twitter,
     * and any hreflang alternates. Begins with a newline so it appends directly
     * after the caller's last head line. Already escaped for attribute context —
     * callers pass it through Vike's `dangerouslySkipEscape`.
     */
    tags: string;
}

/** HTML-escape a value for safe use inside a double-quoted attribute. */
export function attr(value: string): string {
    return value
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

/** Absolute URL for a route path. The home canonical/og:url has no trailing
 * slash (matching the prior Next output); other routes never carry one. */
export function absoluteUrl(siteUrl: string, path: string): string {
    if (!path || path === '/') return siteUrl;
    return `${siteUrl}${path.startsWith('/') ? path : `/${path}`}`;
}

/**
 * Build the head tags every Avant Media site shares, whatever chrome it wears:
 * the title template, canonical, robots directive, Open Graph, and Twitter card.
 *
 * This is the single implementation of those tags. `renderer/+onRenderHtml.tsx`
 * calls it directly and supplies the SpecForge document shell (its own favicons,
 * its own Inter preloads, no hreflang), so the SEO conventions stay shared with
 * the studio sites where the visual identity does not.
 *
 * Deliberately NOT included here, because each is a property of one app rather
 * than of the brand: the favicon set, the `<body>` class, the font-preload
 * allowlist, `<html lang>`, and the hreflang/`og:locale:alternate` pair (passed
 * in by callers that have a counterpart site).
 *
 * The emitted string's leading newline and 8-space indentation are load-bearing:
 * they reproduce the studio sites' existing output byte-for-byte.
 */
export function buildDocumentMeta(
    cfg: DocumentMetaConfig,
    { path, props, hreflangTags = '', ogLocaleAlternate = '' }: DocumentMetaInput,
): DocumentMeta {
    const pageTitle = props?.title ?? cfg.ogSiteName;
    const title = props?.titleAbsolute ? pageTitle : `${pageTitle} | ${cfg.brand}`;
    const description = props?.description ?? '';
    const canonical = absoluteUrl(cfg.siteUrl, path);
    const ogType = props?.ogType ?? (props?.date ? 'article' : 'website');
    const image = props?.image ?? cfg.defaultOgImage;
    const imageUrl = image.url.startsWith('http') ? image.url : `${cfg.siteUrl}${image.url}`;

    // A draft implies noindex, so a preview-emitted draft is never indexable
    // even if the author did not set `noindex` explicitly.
    const robots =
        props?.noindex || props?.draft
            ? '<meta name="robots" content="noindex,follow" />'
            : '<meta name="robots" content="index,follow" />';

    const tags = `
        <link rel="canonical" href="${attr(canonical)}" />
        ${robots}
        <meta name="theme-color" content="${attr(cfg.themeColor)}" />
        <meta property="og:title" content="${attr(title)}" />
        <meta property="og:description" content="${attr(description)}" />
        <meta property="og:type" content="${ogType}" />
        <meta property="og:url" content="${attr(canonical)}" />
        <meta property="og:site_name" content="${attr(cfg.ogSiteName)}" />
        <meta property="og:locale" content="${attr(cfg.ogLocale)}" />${ogLocaleAlternate}
        <meta property="og:image" content="${attr(imageUrl)}" />
        <meta property="og:image:width" content="${image.width}" />
        <meta property="og:image:height" content="${image.height}" />
        <meta property="og:image:alt" content="${attr(image.alt)}" />
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content="${attr(title)}" />
        <meta name="twitter:description" content="${attr(description)}" />
        <meta name="twitter:image" content="${attr(imageUrl)}" />
        ${hreflangTags}`;

    return { title, description, canonical, tags };
}
