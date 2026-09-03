import { transformSync } from 'esbuild';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { Module } from 'node:module';
import { join } from 'node:path';
import type { BuildPage, SiteDocumentProps } from '../postMeta';

/**
 * Build-time reader for the page metadata registry.
 *
 * `import.meta.glob` is unavailable in a Node build plugin, so this transpiles
 * and evaluates each page's component-free `+documentProps.ts` with esbuild —
 * the sitemap and search-index steps then share one mechanism and never
 * hand-maintain a list. Those files import only TYPES from `../postMeta`, which
 * esbuild erases, so the transpiled output has no runtime dependencies.
 */
function evaluateDocumentProps(metaPath: string): SiteDocumentProps | null {
    const source = readFileSync(metaPath, 'utf-8');
    const { code } = transformSync(source, {
        loader: 'ts',
        format: 'cjs',
        target: 'node18',
    });

    const sandbox = new Module(metaPath);
    sandbox.filename = metaPath;
    // @ts-expect-error — private but stable Module internals used for eval.
    sandbox.paths = Module._nodeModulePaths(metaPath);
    // @ts-expect-error — private but stable Module internals used for eval.
    sandbox._compile(code, metaPath);

    // `+documentProps.ts` files export the props object as the default export.
    const props = (sandbox.exports as { default?: SiteDocumentProps }).default;
    return props ?? null;
}

function toBuildPage(props: SiteDocumentProps): BuildPage {
    return {
        url: props.path,
        title: props.title,
        description: props.description,
        date: props.date,
        modified: props.modified ?? props.date,
        category: props.category,
        // A draft implies noindex, so a previewed draft is never indexable.
        noindex: props.noindex || props.draft,
        inSitemap: props.inSitemap !== false && !props.noindex && !props.draft,
        // On-site search is a separate decision from search-engine indexing, but
        // they COINCIDE by default: the usual reason to noindex a page (a legal
        // page for a third-party app, an error page) is also a reason to keep it
        // out of Avant Search. Decoupling is deliberate and explicit — radar
        // blips withheld only for crawl budget pass `inSearchIndex: true`.
        inSearchIndex: props.inSearchIndex ?? !(props.noindex || props.draft),
        draft: props.draft,
    };
}

/**
 * Recursively enumerate every page directory under `pagesDir` that has a
 * `meta.ts`, returning the evaluated metadata for each. Vike parameterised
 * route dirs (`@slug`) carry no static `meta.ts` and are skipped — their routes
 * are supplied separately via `extraRoutes` on the discovery plugin.
 */
export function collectBuildPages(pagesDir: string): BuildPage[] {
    const pages: BuildPage[] = [];

    const walk = (dir: string): void => {
        for (const dirent of readdirSync(dir, { withFileTypes: true })) {
            if (!dirent.isDirectory()) continue;
            // Skip Vike special dirs (parameterised routes, groups).
            if (dirent.name.startsWith('@') || dirent.name.startsWith('(')) {
                continue;
            }
            const childDir = join(dir, dirent.name);
            const metaPath = join(childDir, '+documentProps.ts');
            if (existsSync(metaPath)) {
                const props = evaluateDocumentProps(metaPath);
                if (props && typeof props.title === 'string') {
                    pages.push(toBuildPage(props));
                }
            }
            walk(childDir);
        }
    };

    walk(pagesDir);
    // Stable ordering independent of filesystem enumeration.
    pages.sort((a, b) => a.url.localeCompare(b.url));
    return pages;
}

/** XML-escape a string for safe inclusion in element text or attributes. */
export function escapeXml(value: string): string {
    return value
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&apos;');
}
