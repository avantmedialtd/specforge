# The marketing site

`site/` is the Vike + React static site published at `specforge.avantmedia.uk`. It documents this app's binaries, flags and install routes, which is why it lives here rather than in the studio monorepo it moved out of.

It is **deliberately isolated**: its own `package.json`, `bun.lock` and `node_modules`, and **not** a member of any workspace glob. The root `bun install`, the root `tsc` (`include: ["src"]`) and the root Vite build never touch it, and it resolves its own React/Vite versions rather than the desktop app's.

Two things bite if you forget them (a third, the root `bunfig.toml` test-ignore pattern, is in the root `CLAUDE.md`):

- **Every site command must run with the working directory set to `site/`.** The sitemap and search-index Vite plugins resolve `<cwd>/pages` and write `<cwd>/public`. The root `site:*` scripts all go through `bun run --cwd site` for exactly this reason.
- **`site/site-kit/` is a vendored fork**, not a dependency — see its README for the source commit and the `diff -r` drift check. Do not reintroduce a barrel that re-exports a `Layout`; the upstream ones reach into a design system that was deliberately left behind.

Publishing is `.github/workflows/site.yml`, path-filtered to `site/**` and separate from `ci.yml` on purpose. It stays inert until the `SITE_DEPLOY_ROLE_ARN` repository variable is set, and dry-runs until `SITE_DEPLOY_MODE` is `live`.
