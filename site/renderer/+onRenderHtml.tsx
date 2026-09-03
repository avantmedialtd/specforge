import { buildDocumentMeta } from '../site-kit/documentMeta';
import { PageDataContext } from '../site-kit/PageData';
import type { SiteDocumentProps } from '../site-kit/postMeta';
import type { ComponentType } from 'react';
import ReactDOMServer from 'react-dom/server';
import { dangerouslySkipEscape, escapeInject } from 'vike/server';
import type { OnRenderHtmlAsync } from 'vike/types';
import { Layout } from '../src/Layout';
import { specforgeMetaConfig } from '../src/render-config';
import '../src/styles.css';

/**
 * SpecForge's document renderer.
 *
 * Unlike the UK and HU apps — whose `renderer/` entries are one-line bindings to
 * `createOnRenderHtml` — this app owns its shell, because `createOnRenderHtml`
 * bakes in the studio identity: the brutalist `Layout`, the Avant Media favicon
 * set, `<body class="bg-background">`, the Inter/Archivo font-preload allowlist,
 * and hreflang pairing against a counterpart site.
 *
 * What it does NOT own is the SEO contract. The canonical, robots directive,
 * Open Graph and Twitter tags come from the shared `buildDocumentMeta`, the same
 * function the studio renderer calls, so those conventions cannot drift between
 * the three sites.
 *
 * No hreflang alternates and no `og:locale:alternate` are emitted: this site has
 * no counterpart in another locale, and an alternate pointing at itself would be
 * a lie.
 */
const PRELOADED_FONT_BASENAMES = ['inter-latin-400-normal', 'inter-latin-600-normal'];

/** Keep a font preload only for an above-the-fold Latin woff2 weight. */
function isPreloadedFont(src: string): boolean {
    if (!src.includes('.woff2')) return false;
    return PRELOADED_FONT_BASENAMES.some(name => src.includes(name));
}

export const onRenderHtml: OnRenderHtmlAsync = async pageContext => {
    const Page = pageContext.Page as ComponentType;
    const data = pageContext.data as { documentProps?: SiteDocumentProps } | undefined;
    const props =
        data?.documentProps ??
        (pageContext.config as { documentProps?: SiteDocumentProps })?.documentProps;

    const path = props?.path ?? pageContext.urlPathname ?? '/';

    const pageHtml = ReactDOMServer.renderToString(
        <Layout currentPath={path}>
            <PageDataContext.Provider value={pageContext.data}>
                <Page />
            </PageDataContext.Provider>
        </Layout>,
    );

    const meta = buildDocumentMeta(specforgeMetaConfig, { path, props });
    const headMeta = dangerouslySkipEscape(meta.tags);

    const documentHtml = escapeInject`<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta name="description" content="${meta.description}" />
        <title>${meta.title}</title>
        <link rel="icon" type="image/svg+xml" href="/specforge-icon.svg" />
        <link rel="icon" type="image/png" sizes="64x64" href="/specforge-icon-64.png" />
        <link rel="apple-touch-icon" href="/specforge-icon-180.png" />${headMeta}
    </head>
    <body>
        <div id="root">${dangerouslySkipEscape(pageHtml)}</div>
    </body>
</html>`;

    return {
        documentHtml,
        pageContext: {},
        // Same trimming rationale as the studio apps: keep only the
        // above-the-fold Latin weights this app actually uses, and let every
        // other weight/subset load on demand via its `unicode-range`
        // `@font-face`. The allowlist names Inter because that is what this app
        // sets above the fold — JetBrains Mono appears only in code blocks.
        injectFilter(assets) {
            for (const asset of assets) {
                if (asset.assetType === 'font' && !isPreloadedFont(asset.src)) {
                    asset.inject = false;
                }
            }
        },
    };
};
