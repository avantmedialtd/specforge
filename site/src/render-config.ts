import type { DocumentMetaConfig } from '../site-kit/documentMeta';
import { SITE_URL } from './site-config';

/**
 * The SpecForge site's document-meta configuration.
 *
 * Deliberately a {@link DocumentMetaConfig} and not a `SiteRenderConfig`: this
 * site supplies its own document shell and chrome, so it needs the brand-neutral
 * head inputs and nothing else. There is no `chrome`, because it renders no
 * shared Header/Footer, and no `hreflang`, because the site has no counterpart
 * in another locale.
 */
export const specforgeMetaConfig: DocumentMetaConfig = {
    siteUrl: SITE_URL,
    brand: 'SpecForge',
    ogLocale: 'en_GB',
    ogSiteName: 'SpecForge',
    // The product's dark surface, so a browser's UI chrome matches the app icon
    // rather than the studio's brutalist navy.
    themeColor: '#0a0d12',
    defaultOgImage: {
        url: '/og-specforge.png',
        width: 1200,
        height: 630,
        alt: 'SpecForge showing a change’s tasks beside the workspace tree and commit-graph rail',
    },
};
