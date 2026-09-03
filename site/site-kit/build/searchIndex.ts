import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Plugin } from 'vite';
import type { BuildPage } from '../postMeta';
import { collectBuildPages } from './registry';

interface SearchIndexEntry {
    title: string;
    description: string;
    url: string;
}

export interface SearchIndexConfig {
    /** Extra routes to include (e.g. radar pages), mirroring the sitemap. */
    extraRoutes?: BuildPage[];
}

/**
 * Emit `public/search-index.json` (title / description / url per page) at build
 * start, derived from the shared page registry. The index is emitted for future
 * client-side search; wiring a search UI is a separate change.
 */
export function searchIndexPlugin(cfg: SearchIndexConfig = {}): Plugin {
    return {
        name: 'am-search-index',
        buildStart() {
            const root = process.cwd();
            const pages = [...collectBuildPages(join(root, 'pages')), ...(cfg.extraRoutes ?? [])];
            // Filters on `inSearchIndex`, NOT `noindex`: on-site search and
            // search-engine indexing are separate decisions. A page withheld
            // from Google for crawl-budget reasons must still be findable here.
            // `draft` excludes a page from both regardless.
            const entries: SearchIndexEntry[] = pages
                .filter(p => p.inSearchIndex !== false && !p.draft)
                .map(p => ({
                    title: p.title,
                    description: p.description,
                    url: p.url,
                }));

            entries.sort((a, b) => a.title.localeCompare(b.title));

            writeFileSync(
                join(root, 'public', 'search-index.json'),
                JSON.stringify(entries, null, 2),
            );
            console.log(`✓ Search index: ${entries.length} pages`);
        },
    };
}
