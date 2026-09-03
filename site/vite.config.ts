import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import vike from 'vike/plugin';

// Build-time plugins are imported by relative path from the vendored `site-kit/`
// tree so Vite's esbuild config-bundler inlines them. A bare package specifier
// externalizes them and breaks extensionless ESM resolution — which is also why
// these cannot go through a `resolve.alias`: the config is bundled before any
// alias exists.
import { discoveryArtifactsPlugin } from './site-kit/build/discovery';
import { searchIndexPlugin } from './site-kit/build/searchIndex';

// Kept in step by hand with `SITE_URL` in `src/site-config.ts`. That one feeds
// runtime rendering and JSON-LD; this one feeds the build-time sitemap and
// robots.txt. They must not drift — `e2e/tests/seo.spec.ts` asserts both.
const SITE_URL = 'https://specforge.avantmedia.uk';

export default defineConfig({
    plugins: [
        tailwindcss(),
        searchIndexPlugin(),
        discoveryArtifactsPlugin({
            siteUrl: SITE_URL,
            // No dated articles: this is a product site, not a publication, so
            // there is no feed to emit and `robots.txt` advertises only the
            // sitemap.
            feed: false,
            // Every page here was authored in one sitting, so the sitemap
            // legitimately carries a single `<lastmod>`. The whole-document
            // "more than one distinct date" guard exists to catch a
            // reintroduced constant on the mature studio sites; here it is a
            // false positive. The per-URL rule still applies — a page with no
            // authored `modified` date fails this build exactly as it would the
            // UK one.
            requireDistinctLastmod: false,
        }),
        react(),
        vike(),
    ],
    build: {
        outDir: 'dist',
    },
    // `vike preview` ignores a `--port` flag, so the port is pinned here: the
    // Playwright config's `webServer` waits on exactly this URL. Dev is left on
    // Vike's default 3000, which does not collide with the desktop app's Vite
    // server on 1420 (`../vite.config.ts` pins that one with `strictPort`).
    preview: {
        host: '127.0.0.1',
        port: 4173,
        strictPort: true,
    },
});
