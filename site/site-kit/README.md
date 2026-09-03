# Vendored `site-kit`

These eight modules are a fork of `packages/site-kit/src/` from the Avant Media
studio monorepo (`avantmedialtd/avantmedia`), taken at commit
`bca1b364fc918bc4e14fc2cf4033798f1488f905` when the SpecForge site moved into
this repository.

They are **first-party source now**, not a dependency. Edit them freely.

## What was taken

| File | Provides |
|---|---|
| `postMeta.ts` | `SiteDocumentProps` — the per-page metadata contract every `+documentProps.ts` implements — and `BuildPage` |
| `documentMeta.ts` | `buildDocumentMeta` / `DocumentMetaConfig` — the shared `<head>` builder |
| `PageData.ts` | `PageDataContext`, the Vike `+data` bridge |
| `build/registry.ts` | `collectBuildPages` — esbuild-evaluates every `+documentProps.ts` at build time |
| `build/discovery.ts` | the Vite plugin that writes `public/sitemap.xml` and `public/robots.txt` |
| `build/searchIndex.ts` | the Vite plugin that writes `public/search-index.json` |
| `build/onPrerenderStart.ts` | the draft-filtering Vike prerender hook |
| `build/drafts.ts` | the `INCLUDE_DRAFTS` flag |

## What was deliberately NOT taken

`index.ts`, `server.tsx`, `client.tsx`, `config.ts`, `identity.ts`, `posts.ts`,
`Layout.tsx` and `build/index.ts`.

The first two matter most: they are barrels that re-export a `Layout` which
imports the studio's `@am/ui` design system. In the monorepo that resolved
through an npm-workspaces symlink and was tree-shaken out of the bundle. Copied
here it would simply fail to resolve. **Import these modules directly — do not
reintroduce a barrel that re-exports a Layout.**

Nothing this site uses touches `@am/ui`, so none of its 3,885 lines came across.

## Deviations from upstream

Logic is byte-identical. Only comments were changed, plus one user-visible
string:

- References to `@am/site-kit`, `@am/ui`, `./config`, `./server` and `src/posts.ts`
  were reworded to name modules that exist here.
- `build/discovery.ts` — the error thrown when a page has no authored date
  dropped its `src/radar/blips.ts` clause, which named a route this site
  does not have.

Explanatory comments that cite the studio sites as *history* (why the
`requireDistinctLastmod` guard exists, what `extraRoutes` was for) were kept:
they document why the code is shaped as it is.

## Keeping an eye on upstream

`site-kit` is still evolving in the monorepo for the UK, HU and Meter Burn
sites. To see what has changed since the fork:

```sh
diff -r site/site-kit ../avantmedia/packages/site-kit/src
```

Expect differences confined to the eight files above and the comment edits
listed here. There is no obligation to track upstream — this is a fork, not a
vendored dependency — but the diff is the cheapest way to spot a bug fix worth
copying.
