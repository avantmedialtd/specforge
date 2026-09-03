# Tasks

## 1. Site tree and vendored rendering code

- [x] 1.1 Copy the 45 source files of the studio repo's `apps/specforge` into `site/`, excluding build output, `tsconfig.tsbuildinfo`, the generated `public/{robots.txt,sitemap.xml,search-index.json}`, and the visual-regression suite (`e2e/tests/visual.spec.ts` plus its 18 baselines) (`marketing-site`: *Site lives in this repository and serves its own domain*)
- [x] 1.2 Vendor eight modules from `packages/site-kit/src` into `site/site-kit/` — `PageData.ts`, `postMeta.ts`, `documentMeta.ts`, and `build/{drafts,onPrerenderStart,registry,discovery,searchIndex}.ts` — preserving filenames and the `build/` split (`marketing-site`: *Shared rendering code is vendored, not depended on*)
- [x] 1.3 Do not copy `index.ts`, `server.tsx`, `client.tsx`, `config.ts`, `identity.ts`, `posts.ts`, `Layout.tsx` or `build/index.ts`; the first two re-export a `Layout` that imports the studio design system
- [x] 1.4 Rewrite the 16 module imports in `site/renderer/`, `site/src/render-config.ts` and the ten `site/pages/**/+documentProps.ts` to relative paths into `site/site-kit/`
- [x] 1.5 Repoint `site/vite.config.ts` at `./site-kit/build/discovery` and `./site-kit/build/searchIndex`, drop `ssr.noExternal` and the unused `resolve.alias`, and note the `SITE_URL` duplication with `src/site-config.ts`
- [x] 1.6 Reword comments in the vendored modules and in `site/src/` that name studio packages or routes that do not exist here, including the thrown error in `build/discovery.ts`
- [x] 1.7 Write `site/site-kit/README.md` recording the source commit, what was and was not taken, and the `diff -r` drift check (`marketing-site`: *Shared rendering code is vendored, not depended on*)

## 2. Standalone toolchain

- [x] 2.1 Write `site/package.json` — standalone, adding `esbuild` and `@playwright/test`, dropping `@am/site-kit` (`marketing-site`: *Site lives in this repository and serves its own domain*)
- [x] 2.2 Write `site/tsconfig.json` with the studio's `tsconfig.base.json` inlined, the `@am/site-kit` paths dropped, and `types` widened to `["vite/client", "node"]` so the vendored Node-only build modules type-check
- [x] 2.3 Add `site/.prettierrc` scoped to the site; do not introduce a repository-wide formatter, which this repo has never had
- [x] 2.4 Add `site/.gitignore` for the plugin-generated `public/` artefacts and Playwright output
- [x] 2.5 Port the British-English guard to `site/scripts/check-uk-spelling.mjs`, widening its scope from `pages` to `pages` and `src` (`marketing-site`: *Page copy uses British English*)
- [x] 2.6 Write `site/playwright.config.ts` at the site root with `testDir: './e2e/tests'` and a `vike preview` `webServer`; pin `preview.host`/`preview.port` in `site/vite.config.ts` because `vike preview` ignores a `--port` flag
- [x] 2.7 Derive the third-party-request check in `site/e2e/tests/cookies.spec.ts` from the `baseURL` fixture instead of the studio's Docker hostname (`marketing-site`: *No cookies and no analytics*)
- [x] 2.8 Add the root `bunfig.toml` with `[test] pathIgnorePatterns` so `bun test` in `ci.yml` does not collect the site's Playwright specs (`marketing-site`: *Site lives in this repository and serves its own domain*)
- [x] 2.9 Add `site:dev` / `site:build` / `site:preview` / `site:test` to the root `package.json`, each via `bun run --cwd site` so the build plugins see the right working directory

## 3. Publishing pipeline

- [x] 3.1 Write `.github/workflows/site.yml` with path filters, building and testing the site and uploading `site/dist/client` with `include-hidden-files: true` (`marketing-site`: *Publishing is path-filtered, gated and explicitly armed*)
- [x] 3.2 Gate the deploy job on `vars.SITE_DEPLOY_ROLE_ARN` being set, and on `SITE_DEPLOY_MODE` being `live` for a real sync; dry-run otherwise, probing `cloudfront get-invalidation` to prove the grant
- [x] 3.3 Assert the artefact's shape — six required documents non-empty and at least 30 files — before any `aws s3 sync --delete`
- [x] 3.4 Port the post-deploy smoke check to `site/scripts/post-deploy-check.sh`, deriving routes from the deployed sitemap rather than a hand-maintained list (`marketing-site`: *Publishing is path-filtered, gated and explicitly armed*)
- [x] 3.5 Set the `SITE_DEPLOY_ROLE_ARN` repository variable, confirm a green dry run, then set `SITE_DEPLOY_MODE=live` and confirm a real publish

## 4. Repository documentation

- [x] 4.1 Add a `site/` section and command-table entries to `CLAUDE.md`
- [x] 4.2 Extend the `context:` block in `openspec/config.yaml` to describe `site/`, its isolation and the working-directory invariant, since that block is injected into every future artifact-creation prompt

## 5. AWS groundwork (studio repository, deployed by hand)

Landed on the studio repo's master as `specforge-site-deploy-role` and deployed by hand. Proven by the first dry run: the role was assumed over OIDC, and the CloudFront probe returned `NoSuchInvalidation` rather than `AccessDenied`, which is the signal that the grant exists.

- [x] 5.1 In the studio repo's `cloud/lib/Website.ts`, add an opt-in `versioned` prop passed to the `Bucket`, so a bad publish is recoverable
- [x] 5.2 In `cloud/lib/WebStack.ts`, capture the `WebsiteSpecForge` construct, set `versioned: true`, and add a GitHub OIDC provider plus a deploy role trusting this repository's master branch, scoped to the one bucket and the one distribution; output the role ARN
- [x] 5.3 Run `aws iam list-open-id-connect-providers` first — an existing GitHub provider must be imported rather than created
- [x] 5.4 Run `npm run diff` and confirm it proposes only the provider, the role, its custom resource and bucket versioning; **stop if it proposes any change to the bucket beyond versioning, to a distribution, or to the certificate**
- [x] 5.5 Deploy the single stack with `--require-approval broadening`; never the repo's `deploy` script, which is `cdk deploy --all --require-approval never`

## 6. Studio-repository removal (lands there, after group 3.5)

- [ ] 6.1 Delete `apps/specforge/` and its four `package.json` scripts
- [ ] 6.2 Remove the `Jenkinsfile` distribution-map entry, build branch and deploy target
- [ ] 6.3 Remove the `Dockerfile` manifest copy, the `main-specforge` compose service and the test-compose mount
- [ ] 6.4 Remove both SpecForge projects from `e2e/playwright.config.ts`
- [ ] 6.5 Remove `apps/specforge/pages` from `scripts/check-uk-spelling.mjs` and the SpecForge branches of `scripts/post-deploy-test.sh`
- [ ] 6.6 Reword the dangling path reference in `apps/meterburn/e2e/tests/cookies.spec.ts`; leave the `seo.spec.ts` negative guard, which stays valid because the domain survives
- [ ] 6.7 Write the studio-side OpenSpec change: REMOVED delta for `specforge-site`, MODIFIED deltas for `monorepo-structure`, `dual-site-infrastructure` and `meterburn-site` (which becomes the third Vike application, not the fourth)
- [ ] 6.8 **Do not touch** `cloud/lib/WebStack.ts`'s `WebsiteSpecForge` construct — it owns the live bucket and distribution — or the `/portfolio/specforge` case study and the UK tests asserting it

## 7. Verification

- [x] 7.1 `bun run --cwd site build` succeeds; `dist/client/sitemap.xml` lists exactly nine URLs, `robots.txt` advertises the sitemap, and no `feed.xml` is emitted
- [x] 7.2 `bun run --cwd site typecheck` passes
- [x] 7.3 `bun run --cwd site test:e2e` — all six functional specs green across both viewports (122 tests)
- [x] 7.4 `node site/scripts/check-uk-spelling.mjs` passes
- [x] 7.5 `bun test` at the repository root discovers the same 22 files and 383 tests as before the move, and `bun test --path-ignore-patterns` with a non-matching pattern shows 28, proving the `bunfig.toml` is load-bearing
- [x] 7.6 `bun install --frozen-lockfile` and `bun run build` at the repository root both succeed with no lockfile change
- [x] 7.7 `diff -r site/site-kit` against the upstream package shows only the eight vendored files, the added README, and no non-comment change beyond the one documented error string
- [x] 7.8 `site/scripts/post-deploy-check.sh` passes against the live site as currently deployed by Jenkins
- [x] 7.9 Dry-run deploy green (role assumed; the CloudFront probe returned `NoSuchInvalidation`, not `AccessDenied`, proving the grant), then a live publish of 86 files with 15 stale hashed chunks deleted; all nine routes serve 200 and every asset the live page references resolves
