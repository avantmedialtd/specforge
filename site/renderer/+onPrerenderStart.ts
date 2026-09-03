// Thin entry: the draft-filtering prerender logic lives in `site-kit/build/`.
// This registers it as the site's global Vike onPrerenderStart hook so a default
// build omits draft routes (INCLUDE_DRAFTS=1 keeps them for preview).
export { onPrerenderStart } from '../site-kit/build/onPrerenderStart';
