# Move the Marketing Site Into This Repository

## Why

The SpecForge marketing site — `specforge.avantmedia.uk` — has lived in the Avant Media studio monorepo since it launched, as a fourth Vike application beside the two studio sites and Meter Burn. Its eight documentation pages describe *this* repository's artefacts: the dashboard, the commit graph, the terminal UI, `specforge-serve`, the settings surface, the `xattr -dr com.apple.quarantine` incantations, the `@avantmedia/specforge` npm route and the platform matrix of every release bundle.

That split makes the docs structurally lag the product. A flag renamed here, a platform added to the release matrix, a port changed — each requires a second change in a second repository that nothing forces anyone to make. The evidence is already in the specs: the site's own governing capability in the studio repo still describes seven routes and an H1 the site stopped using, because the last three site changes shipped with no spec delta at all.

Co-locating the site with the product it documents makes the docs updatable in the same commit as the behaviour they describe.

## What Changes

```mermaid
flowchart LR
    subgraph before [Before]
        A[avantmedia monorepo] -->|npm workspace| B[apps/specforge]
        B -->|@am/site-kit| C[packages/site-kit]
        C -.->|barrel leak| D[packages/ui]
        B -->|Jenkins| E[(S3 + CloudFront)]
    end
    subgraph after [After]
        F[specforge repo] --> G[site/]
        G -->|vendored, 8 files| H[site/site-kit/]
        G -->|GitHub Actions + OIDC| E2[(same S3 + CloudFront)]
    end
```

- **The site moves to `site/`** — all 45 source files of `apps/specforge`, unchanged apart from import rewrites. Nine routes, a prerendered 404, and a self-contained Tailwind v4 stylesheet that already carried its own visual identity.
- **The shared `@am/site-kit` dependency is vendored, not depended on.** The site used exactly five symbols from it. Those are backed by eight modules totalling ~716 lines, now first-party source under `site/site-kit/`. Nothing from the studio's `@am/ui` design system came across — the site imported none of its 3,885 lines, and its `index.ts`/`server.tsx` barrels are deliberately *not* copied because they re-export a `Layout` that reaches into `@am/ui`.
- **`site/` is standalone, not a workspace member.** Its own `package.json`, `bun.lock` and `node_modules`; its own `tsconfig.json` with the studio's `tsconfig.base.json` inlined. The root `bun install --frozen-lockfile`, the root `tsc` (`include: ["src"]`) and the root Vite build are all untouched by it.
- **A root `bunfig.toml` is added.** `bun test` matches `*.spec.ts` as well as `*.test.ts`, so without it the root test job in `ci.yml` collects the site's Playwright specs and dies on the first `test.describe()`. `[test] pathIgnorePatterns` keeps root discovery at exactly the 22 files it covers today.
- **Publishing moves to a new `site.yml` workflow** — path-filtered, building and testing the site and then syncing to the same bucket and invalidating the same CloudFront distribution Jenkins uses today, authenticated by **OIDC role assumption** rather than the long-lived studio IAM user. It publishes only once a deploy role ARN and an explicit `live` mode are set, so it lands inert.
- **The visual-regression suite does not come across.** Its 18 committed PNG baselines were generated in the studio's amd64 Docker/nginx harness; reproducing that harness here to keep byte-compatible screenshots is not worth its weight. The six functional specs — routes, SEO, downloads, landing, layout, cookies — all move, and now run against `vike preview` with no Docker at all.
- **The British-English guard comes across** and widens from `pages` to `pages` + `src`. It exists because `id="personalization"` once shipped on this site; the header and footer copy it did not previously cover ships just as visibly.

Removing `apps/specforge` from the studio monorepo is the second half of this work and lands there, not here. It must follow a verified publish from this repository — until then Jenkins remains the only thing deploying the site.

## Capabilities

### New Capabilities

- `marketing-site`: the site's routes, its release-process coupling (never naming a version), its publishing pipeline and its no-cookies posture. Carried across from the studio repo's `specforge-site` capability and corrected to the site as it actually ships — nine routes, not seven — with the requirements that only made sense inside a monorepo reframed.

### Modified Capabilities

_None._ The `continuous-integration` capability is deliberately untouched: its normative trigger and five jobs describe `ci.yml`, and the site gets its own workflow rather than a sixth job there.

## Impact

- **New**: `site/` (59 files), `.github/workflows/site.yml`, `bunfig.toml`.
- **Modified**: `package.json` (four `site:*` convenience scripts, all routed through `bun run --cwd site` because the build plugins resolve `<cwd>/pages` and write `<cwd>/public`), `CLAUDE.md`, `openspec/config.yaml` (the `context:` block is injected into every future artifact-creation prompt and must mention `site/`).
- **No Rust, no IPC, no dependency change** in the desktop app. `openspec-core`/`openspec-app` are untouched, so the mutation gate short-circuits; coverage for the site comes from its own typecheck, build, British-English guard and 122-test Playwright suite in `site.yml`.
- **`ci.yml` has no path filters**, so a site-only push still runs the full Rust and Tauri matrix. That is wasted compute, not a failure, and narrowing it would be a `continuous-integration` spec change — deliberately out of scope here.
- **Requires one AWS change outside this repository** before anything publishes: a GitHub OIDC provider and a deploy role scoped to this repo's master branch, plus versioning on the bucket. The CDK stack that owns the bucket, the distribution and the wildcard certificate stays in the studio repo.
