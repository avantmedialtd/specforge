import type { SiteDocumentProps } from '../postMeta';
import { includeDrafts } from './drafts';

/** The minimal slice of each page's prerender context this hook reads. */
interface PrerenderPageContext {
    config?: { documentProps?: SiteDocumentProps };
}

/**
 * Global Vike `onPrerenderStart` hook: drop draft pages from the prerender set
 * unless this is an `INCLUDE_DRAFTS=1` (preview) build. A page's draft status is
 * read from its server-only `documentProps` — the same source the renderer and
 * the discovery plugins use — so `draft: true` in `+documentProps.ts` is the
 * single switch, with no per-page config or separate manifest.
 *
 * Filtering a page out here prevents its prerendering, so no HTML document is
 * emitted for it in a default build (the route 404s in production). Drafts still
 * render under `vike dev` — this hook runs only during prerendering — and are
 * excluded from the discovery artifacts unconditionally (see registry/discovery).
 *
 * Kept in its own module, separate from the discovery plugins, so the app's
 * `renderer/+onPrerenderStart.ts` entry does not pull `esbuild`/`fs` through Vite.
 */
export function onPrerenderStart<T extends PrerenderPageContext>(prerenderContext: {
    pageContexts: T[];
}): { prerenderContext: { pageContexts: T[] } } | undefined {
    if (includeDrafts) return undefined;
    const pageContexts = prerenderContext.pageContexts.filter(
        pc => !pc.config?.documentProps?.draft,
    );
    // Nothing to drop (no drafts, or a build with all drafts published): return
    // undefined so Vike keeps the original set untouched. Returning a list only
    // when we actually exclude a draft also confines a benign Vike deprecation
    // nag — Vike's own back-compat code reads the deprecated `pageContext.url`
    // on a returned list (runPrerender, "TO-DO/next-major-release: remove"),
    // tripping the getter Vike installed — to builds that genuinely have drafts.
    if (pageContexts.length === prerenderContext.pageContexts.length) return undefined;
    // Return only `pageContexts` — never spread `prerenderContext`, which would
    // enumerate Vike's internal `_`-prefixed fields and trip its internals
    // warnings. The filtered entries are the original, unmodified contexts.
    return { prerenderContext: { pageContexts } };
}
