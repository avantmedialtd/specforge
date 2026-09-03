# Design

## Context

The site is a Vike + React 19 + Tailwind v4 static prerender, built as an npm-workspace member of the Avant Media monorepo and published by a Jenkins stage that runs `aws s3 sync --delete` into the bucket `specforge.avantmedia.uk` and invalidates CloudFront distribution `E76R07VP6C80L`. The bucket, the distribution and the `*.avantmedia.uk` wildcard certificate are all owned by a CDK stack in that repository, and the DNS record was created by hand because there is no Route53 hosted zone for the domain.

Three properties of the target repository shape everything below. It is **not** a bun workspace — the root `package.json` has no `workspaces` key — so a nested `package.json` is invisible to the root install. Its root `tsconfig.json` is `include: ["src"]`, so nothing outside `src/` is type-checked by the root build. And its `ci.yml` has no path filters, with its trigger and five jobs fixed normatively by the `continuous-integration` capability.

## Goals / Non-Goals

**Goals.** Move the site without behavioural change. Leave the desktop app's build, test and release pipelines provably untouched. Keep the live site publishable at every point in the sequence. Make the vendored code auditable against the package it forked from.

**Non-Goals.** Redesigning the site, changing its content, or altering its URLs. Moving the AWS infrastructure — only the publishing step moves. Preserving the site's git history in this repository; it stays findable in the studio repo, and the move commit names the source SHA.

## Decisions

### Vendor the used subset of `@am/site-kit`, rather than depending on it

The site imports five symbols: `SiteDocumentProps`, `PageDataContext`, `buildDocumentMeta`, `DocumentMetaConfig`, and the `onPrerenderStart` hook, plus two Vite build plugins imported by relative path. Those are backed by eight modules; everything else in the package serves the studio sites.

The eight are copied to `site/site-kit/` with their filenames and their flat-versus-`build/` split preserved, so `diff -r site/site-kit ../avantmedia/packages/site-kit/src` stays a meaningful drift check. Every import inside them was already relative and needed no edit. Only comments changed, plus one thrown error message that named a route this site does not have.

**The barrels are deliberately not copied.** `index.ts` and `server.tsx` re-export a `Layout` that imports `@am/ui`. In the monorepo that resolved through an npm-workspaces symlink and was tree-shaken out of the bundle; copied here it would simply fail to resolve. The consuming imports therefore point at concrete modules, never a barrel.

*Rejected: publishing `@am/site-kit` to npm and depending on it.* One source of truth, but it adds a release process to a package with no build step, and couples a product site's every change to a studio repository's publish cadence. The coupling being removed is the whole point of the move.

*Rejected: vendoring both packages wholesale.* `@am/ui` is 3,885 lines the site imports none of.

### Place the vendored tree at `site/site-kit/`, not `site/src/site-kit/`

`build/registry.ts`, `build/discovery.ts` and `build/searchIndex.ts` reach `node:fs`, `node:module` and `esbuild`, and must never enter the client bundle. Keeping them outside the directory the renderer imports from makes that boundary structural rather than conventional. It also keeps the upstream diff a flat comparison.

*Rejected: `site/src/site-kit/`.* `src/styles.css` declares `@source '../src/**/*.{ts,tsx}'`, which would pull Node-only build code into Tailwind's class-candidate scan on every build for no benefit.

### Keep `site/` outside any workspace glob

The site gets its own `package.json`, `bun.lock` and `node_modules`. Verified: the root `bun install --frozen-lockfile` stays green with no lockfile change, and the site resolves its own `vite` rather than the root's.

*Rejected: adding `"workspaces": ["site"]` to the root manifest.* It forces a `bun.lock` regeneration that must be committed in the same commit or all seven `--frozen-lockfile` steps across `ci.yml` and `release.yml` fail, and it hoists the site's dependencies into the root `node_modules` where they could shadow the desktop app's `react` and `vite`. The two trees legitimately want different major versions.

### Fix the `bun test` collision with a root `bunfig.toml`

Bun's default test glob matches `*.spec.ts`, not only `*.test.ts`. Confirmed empirically: `bun test` over the site's e2e directory fails with *"Playwright Test did not expect test.describe() to be called here"*, and root discovery rises from 22 files to 28 when the site is present without an ignore. `ci.yml` runs a bare `bun test`, which is off-limits.

`[test] pathIgnorePatterns = ["**/site/**"]` restores discovery to exactly 22 files and 383 tests.

*Rejected: renaming the six specs to `*.e2e.ts`.* It works, but it makes the moved files differ from their source for a reason that has nothing to do with the site, and it needs a matching `testMatch` in the Playwright config — a second place to get wrong. *Rejected: `[test] root = "src"`*, which would also drop `scripts/` and `npm/` from root coverage.

### A separate `site.yml`, publishing only when explicitly armed

The site gets its own path-filtered workflow rather than a sixth job in `ci.yml`, whose shape is normative. Two independent gates stand between landing this change and a real publish: the deploy job is skipped entirely unless the repository variable `SITE_DEPLOY_ROLE_ARN` is set, and even then it runs `aws s3 sync --dryrun` unless `SITE_DEPLOY_MODE` is `live`. In dry-run it also probes `cloudfront get-invalidation`, where `AccessDenied` and `NoSuchInvalidation` are distinguishable — so the IAM grant can be proven before anything is written.

This matters because Jenkins is the *only* publisher today. Removing it before this pipeline has authenticated even once would leave the site frozen with no way to publish, and an OIDC misconfiguration surfaces as an opaque `sts:AssumeRoleWithWebIdentity` error.

### Gate the payload before `aws s3 sync --delete`

The bucket has no versioning today, and a sync from a thin artifact is an unrecoverable outage. Two specific hazards are closed: `actions/upload-artifact` defaults to `include-hidden-files: false` while a Vike build emits a `.vite/` directory, so the artifact would silently lose files the sync then deletes; and nothing otherwise asserts the artifact is a site at all. The deploy job requires six named files to be non-empty and a floor of 30 files (a real build is 86) before it will sync. Bucket versioning is added on the AWS side in the same piece of work.

### Drop the visual-regression suite

Its 18 baselines were generated in an amd64 Docker/nginx harness, and the studio's own conventions require regenerating them there because arm64 anti-aliasing drifts. Reproducing that harness to keep byte-compatible screenshots costs a Docker stack and 6.2 MB of PNGs for a site whose layout is already asserted structurally — `layout.spec.ts` checks overflow at three widths, `landing.spec.ts` checks the header lockup geometry at seven.

*Rejected: porting the harness and regenerating baselines.* Defensible, but it makes the Playwright run depend on Docker and a pinned browser image, for regression coverage the functional specs largely duplicate.

## Risks / Trade-offs

- **The fork will drift from upstream `site-kit`.** Accepted: that is what forking means. `site/site-kit/README.md` records the source SHA, the deviations, and the `diff -r` command for spotting a bug fix worth copying.
- **`registry.ts` uses private Node internals** (`Module._nodeModulePaths`, `sandbox._compile`) to evaluate `+documentProps.ts` at build time. This is the most fragile thing being vendored and it is inherited as-is; Bun implements these, and the build is verified green.
- **`SITE_URL` is declared twice** — in `vite.config.ts` for the build-time sitemap and in `src/site-config.ts` for runtime rendering. Kept as-is to hold the move to a move; a comment now names the coupling, and `e2e/tests/seo.spec.ts` asserts both.
- **Two publishers exist between landing this and removing the Jenkins stage.** Both build the same content from near-identical sources, so a divergence would have to be authored deliberately. The window should be short.
- **A site-only push still runs the full Rust and Tauri CI matrix.** Wasted compute; narrowing it is a `continuous-integration` spec change.
