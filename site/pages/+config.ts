import type { Config } from 'vike/types';

// Global Vike config for the SpecForge app. Prerender every page to static HTML.
// `documentProps` is a server-only custom config provided per page via a
// `+documentProps.ts` file and read by the renderer from pageContext.config.
//
// `prerender.partial` mirrors the studio apps: `renderer/+onPrerenderStart.ts`
// filters draft pages out of the prerender set on a default build, so not every
// route is prerendered and Vike's "cannot pre-render" warning would otherwise
// fire on a draft.
export default {
    prerender: { partial: true },
    passToClient: ['data'],
    meta: {
        documentProps: {
            env: { server: true },
        },
    },
} satisfies Config;
