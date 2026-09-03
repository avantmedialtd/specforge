import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Plugin } from 'vite';
import type { BuildPage } from '../postMeta';
import { collectBuildPages, escapeXml } from './registry';

export interface DiscoveryConfig {
    /** Production origin, e.g. `https://www.avantmedia.uk`. No trailing slash. */
    siteUrl: string;
    /** RSS channel title. Required unless {@link DiscoveryConfig.feed} is `false`. */
    feedTitle?: string;
    /** RSS channel description. Required unless {@link DiscoveryConfig.feed} is `false`. */
    feedDescription?: string;
    /** Two-letter feed language, e.g. `en` or `hu`. Required unless {@link DiscoveryConfig.feed} is `false`. */
    feedLanguage?: string;
    /**
     * Extra routes not discoverable from the `meta.ts` glob — e.g. the radar
     * blip and edition pages, enumerated by the app from its data module.
     */
    extraRoutes?: BuildPage[];
    /**
     * Emit `feed.xml`. Defaults to `true`. Set `false` for a site that publishes
     * no dated articles, so it ships no item-less feed. `robots.txt` advertises
     * only the sitemap either way.
     */
    feed?: boolean;
    /**
     * Fail the build when every sitemap URL shares one `<lastmod>`. Defaults to
     * `true`.
     *
     * On a site with years of authored dates, one date across every URL is the
     * signature of a reintroduced constant — the defect this guard was written
     * for. On a site whose entire content genuinely landed on one day, it is a
     * false positive, and the `sitemap` spec is explicit that "uniformity SHALL
     * NOT be treated as a defect in itself".
     *
     * Disabling this does NOT relax the per-URL rule that every `<lastmod>`
     * traces to an authored date: {@link resolveLastmod} still fails the build
     * on a dateless page, unconditionally, for every app.
     */
    requireDistinctLastmod?: boolean;
}

function absoluteUrl(siteUrl: string, path: string): string {
    // Home carries its trailing slash, matching the canonical the page emits.
    return path === '/' ? `${siteUrl}/` : `${siteUrl}${path}`;
}

/**
 * Assert a `<lastmod>` is a W3C `YYYY-MM-DD` date, a real calendar day, and not
 * in the future. The clock bounds this check only — it never supplies a value.
 */
function assertValidLastmod(url: string, value: string): void {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
        throw new Error(`[discovery] ${url}: lastmod "${value}" is not a YYYY-MM-DD date.`);
    }
    const parsed = new Date(`${value}T00:00:00Z`);
    if (Number.isNaN(parsed.getTime()) || parsed.toISOString().slice(0, 10) !== value) {
        throw new Error(`[discovery] ${url}: lastmod "${value}" is not a real calendar date.`);
    }
    if (value > new Date().toISOString().slice(0, 10)) {
        throw new Error(`[discovery] ${url}: lastmod "${value}" is in the future.`);
    }
}

/**
 * A page's `<lastmod>`: its own authored content date, never a constant and
 * never the build time. A page with no date cannot be emitted — the build fails
 * naming it rather than inventing a value. (The defect this replaced was a fixed
 * `2026-06-01` on 110 of 123 URLs, a date on which nothing happened; Google
 * discounts a `lastmod` it cannot corroborate against the content it fetches.)
 */
function resolveLastmod(page: BuildPage): string {
    const value = page.modified ?? page.date;
    if (!value) {
        throw new Error(
            `[discovery] ${page.url} is in the sitemap but carries no \`modified\` or \`date\`. ` +
                `Add an authored ISO date to its \`+documentProps.ts\`.`,
        );
    }
    assertValidLastmod(page.url, value);
    return value;
}

/** Dated article pages, newest-first with a deterministic slug tie-break. */
function datedPosts(pages: BuildPage[]): (BuildPage & { date: string })[] {
    return pages
        .filter((p): p is BuildPage & { date: string } => typeof p.date === 'string' && !p.draft)
        .sort((a, b) =>
            a.date !== b.date ? (a.date < b.date ? 1 : -1) : a.url.localeCompare(b.url),
        );
}

/** RSS 2.0 pubDate (RFC 822) from an ISO `YYYY-MM-DD` date. */
function toRfc822(isoDate: string): string {
    return new Date(`${isoDate}T00:00:00Z`).toUTCString();
}

function buildFeedXml(cfg: DiscoveryConfig, pages: BuildPage[]): string {
    // Optional on the type so a site with `feed: false` need not invent channel
    // metadata it will never emit; mandatory the moment a feed IS emitted, so a
    // feed-publishing app cannot silently ship an untitled channel.
    if (!cfg.feedTitle || !cfg.feedDescription || !cfg.feedLanguage) {
        throw new Error(
            '[discovery] feedTitle, feedDescription and feedLanguage are required when emitting ' +
                'feed.xml. Supply them, or set `feed: false` for a site with no dated articles.',
        );
    }

    const items = datedPosts(pages)
        .map(post => {
            const link = absoluteUrl(cfg.siteUrl, post.url);
            return `        <item>
            <title>${escapeXml(post.title)}</title>
            <link>${escapeXml(link)}</link>
            <guid isPermaLink="true">${escapeXml(link)}</guid>
            <description>${escapeXml(post.description)}</description>
            <pubDate>${escapeXml(toRfc822(post.date))}</pubDate>
        </item>`;
        })
        .join('\n');

    return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
    <channel>
        <title>${escapeXml(cfg.feedTitle)}</title>
        <link>${escapeXml(absoluteUrl(cfg.siteUrl, '/'))}</link>
        <description>${escapeXml(cfg.feedDescription)}</description>
        <language>${escapeXml(cfg.feedLanguage)}</language>
        <atom:link href="${escapeXml(`${cfg.siteUrl}/feed.xml`)}" rel="self" type="application/rss+xml" />
${items}
    </channel>
</rss>
`;
}

function buildSitemapXml(cfg: DiscoveryConfig, pages: BuildPage[]): string {
    const urls = pages
        .filter(p => p.inSitemap && !p.draft)
        .slice()
        .sort((a, b) => a.url.localeCompare(b.url))
        .map(page => {
            // Two children only. `<changefreq>` and `<priority>` are omitted
            // deliberately — Google ignores both, so computing them was work
            // that bought nothing.
            const loc = absoluteUrl(cfg.siteUrl, page.url);
            return `    <url>
        <loc>${escapeXml(loc)}</loc>
        <lastmod>${escapeXml(resolveLastmod(page))}</lastmod>
    </url>`;
        })
        .join('\n');

    return `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls}
</urlset>
`;
}

/**
 * Whole-document guard on the generated sitemap, run before it is written.
 *
 * The defect this change fixed was a generator emitting one invented date on
 * 110 of 123 URLs, so the guard targets that failure mode directly: a
 * reintroduced constant collapses `distinct` to 1 and fails here.
 *
 * Deliberately NOT asserted: "no single date accounts for more than N% of
 * URLs". 90 of the 96 radar pages genuinely last changed on 2026-02-20, so that
 * rule could only be satisfied by fabricating dates — the very thing being
 * removed. Google corroborates `lastmod` against the content it fetches;
 * shared-but-accurate survives that check, invented-but-varied does not.
 */
function assertSitemapIntegrity(cfg: DiscoveryConfig, xml: string, pages: BuildPage[]): void {
    const fail = (message: string): never => {
        throw new Error(`[discovery] sitemap: ${message}`);
    };

    const lastmods = [...xml.matchAll(/<lastmod>([^<]*)<\/lastmod>/g)].map(m => m[1]);
    const locs = [...xml.matchAll(/<loc>([^<]*)<\/loc>/g)].map(m => m[1]);

    if (locs.length === 0) fail('no <url> entries were emitted.');
    if (locs.length !== lastmods.length) {
        fail(`${locs.length} <loc> but ${lastmods.length} <lastmod> — every URL needs exactly one.`);
    }
    if (cfg.requireDistinctLastmod !== false && new Set(lastmods).size < 2) {
        fail(
            `every URL shares the lastmod "${lastmods[0]}". That is the signature of a constant ` +
                `or build-time stamp; each URL must carry its own content date. If this site's ` +
                `content genuinely all landed on one day, set \`requireDistinctLastmod: false\`.`,
        );
    }
    if (xml.includes('<changefreq>') || xml.includes('<priority>')) {
        fail('<changefreq>/<priority> are ignored by Google and must not be emitted.');
    }
    if (!locs.includes(`${cfg.siteUrl}/`)) {
        fail(`the home entry must be "${cfg.siteUrl}/" (with the trailing slash) to match the canonical.`);
    }

    // A noindex URL in a sitemap is a contradictory signal: it asks a crawler to
    // fetch a page it is simultaneously told not to index.
    const noindexed = pages.filter(p => p.inSitemap && (p.noindex || p.draft));
    if (noindexed.length > 0) {
        fail(`${noindexed.map(p => p.url).join(', ')} are listed but marked noindex/draft.`);
    }
}

function buildRobotsTxt(cfg: DiscoveryConfig): string {
    return `User-agent: *
Allow: /

Sitemap: ${cfg.siteUrl}/sitemap.xml
`;
}

/**
 * Emit the static discovery artifacts (`sitemap.xml`, `robots.txt`, `feed.xml`)
 * into `public/` at build start, derived from the shared build-time page
 * registry plus any app-supplied `extraRoutes` (radar). Vite copies `public/`
 * into the prerendered output. Never a hand-maintained list.
 */
export function discoveryArtifactsPlugin(cfg: DiscoveryConfig): Plugin {
    return {
        name: 'am-discovery-artifacts',
        buildStart() {
            const root = process.cwd();
            const pages = [...collectBuildPages(join(root, 'pages')), ...(cfg.extraRoutes ?? [])];
            const publicDir = join(root, 'public');

            const sitemapXml = buildSitemapXml(cfg, pages);
            assertSitemapIntegrity(cfg, sitemapXml, pages);

            writeFileSync(join(publicDir, 'sitemap.xml'), sitemapXml);
            writeFileSync(join(publicDir, 'robots.txt'), buildRobotsTxt(cfg));

            const emitFeed = cfg.feed !== false;
            if (emitFeed) {
                writeFileSync(join(publicDir, 'feed.xml'), buildFeedXml(cfg, pages));
            }

            const sitemapCount = pages.filter(p => p.inSitemap).length;
            const distinctDates = new Set(
                [...sitemapXml.matchAll(/<lastmod>([^<]*)<\/lastmod>/g)].map(m => m[1]),
            ).size;
            const feedSummary = emitFeed ? `${datedPosts(pages).length} feed items` : 'no feed';
            console.log(
                `✓ Discovery artifacts: ${sitemapCount} sitemap routes (${distinctDates} distinct lastmod), ${feedSummary}`,
            );
        },
    };
}
