// Whether the current build includes draft pages. A default `vike build` leaves
// `INCLUDE_DRAFTS` unset, so drafts are dropped from the prerender set (see each
// app's `renderer/+onPrerenderStart.ts`). An opt-in `INCLUDE_DRAFTS=1` build
// (the `build:preview` script) prerenders them. Evaluated once at module load —
// the env var is set on the whole build process before any module loads.
//
// Lives in the Node-only `site-kit/build/` tree so this `process.env` read
// never reaches the client bundle. Drafts are excluded from discovery artifacts
// and the post registry unconditionally; this flag gates page emission only.
export const includeDrafts = process.env.INCLUDE_DRAFTS === '1';
