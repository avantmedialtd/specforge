import { loadChangelog, type Changelog } from '../../build/releaseNotes';
import { RELEASE_VERSION } from '../../src/site-config';

/**
 * The changelog page's content, assembled while building.
 *
 * A Vike `+data` hook runs on the server — here, during prerender — which is
 * what keeps the markdown parser out of the client graph. The page hydrates
 * fully with no islands, so a parser imported by `+Page.tsx` instead would
 * execute in every visitor's browser.
 *
 * The release this renders is whichever one `site-config.ts` advertises, so the
 * page cannot drift from the downloads block above it: both read one constant,
 * written by `/release` into the same commit as the notes file.
 */
export type Data = Changelog;

export function data(): Data {
    return loadChangelog(RELEASE_VERSION);
}
