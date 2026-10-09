## 1. Shell and packaging metadata

- [x] 1.1 In `crates/specforge/tauri.conf.json`, set `bundle.shortDescription` to "SpecForge — follow OpenSpec changes from proposal to pull request, across your repositories" and `bundle.longDescription` to the copy in design.md D8 (`product-identity`: *Positioning Is Consistent Across Product Surfaces*).
- [x] 1.2 In `crates/specforge/src/menu.rs:61`, change the credits tagline to "Follow OpenSpec changes from proposal to pull request, across your repositories." Confirm the comment at L57-59 still describes it accurately (`product-identity`: *About Panel States Product and Format*).
- [x] 1.3 In `npm/packaging.mjs:130`, set the wrapper `description` to "Run the SpecForge web server: follow OpenSpec changes from proposal to pull request, in a browser." Then run `bun test npm/` and update any assertion that pins the old string (`product-identity`: *Positioning Is Consistent Across Product Surfaces*).

## 2. README and agent context

- [x] 2.1 Replace the bold headline at `README.md:7` with the D8 headline (`product-identity`: *Positioning Is Consistent Across Product Surfaces*).
- [x] 2.2 Replace the introduction at `README.md:19` with the D8 intro. Keep the OpenSpec link and the `openspec/` live-example link.
- [x] 2.3 Replace the second Why paragraph at `README.md:33` with the D8 text. This drops "A dedicated menu-bar app" and the "read-only in v1" hedge.
- [x] 2.4 In Features (`README.md:35-48`), add the Dashboard, Pull requests (opt-in) and Image diffs bullets after *Live commit graph*, and move *One badge for every project*, *Desktop notifications* and *macOS Dock badge* below *Always live*.
- [x] 2.5 Correct the network sentence at `README.md:47` and the Settings toggle list at `README.md:73` as design.md D4 describes (`product-identity`: *Positioning Is Consistent Across Product Surfaces*, scenario *README states network access accurately*).
- [x] 2.6 Replace the first sentence of `context` in `openspec/config.yaml` with the D8 text, leaving the rest of the block unchanged (design.md D6).
- [x] 2.7 Grep the repository for "menu-bar viewer", "visual companion", "lives in your menu bar" and "browses OpenSpec workspaces", excluding `openspec/changes/archive/`, `releases/` and this change, and confirm every remaining hit is intentional.

## 3. Marketing site

- [x] 3.1 In `site/pages/index/+documentProps.ts`, set `title` to "SpecForge — Spec-driven work, from proposal to pull request", set `description` to the D8 meta description, and set `modified` to the implementation date (`marketing-site`: *Landing page states the product's positioning*).
- [x] 3.2 In `site/pages/index/+Page.tsx`, change the hero kicker to "A read-only companion for spec-driven development", the H1 to "Spec-driven work, from proposal to pull request.", and the hero summary to the D8 text. Keep "supports OpenSpec today" in the summary.
- [x] 3.3 In the same file's "One change, end to end" section, change the H2 to "Follow the thinking all the way to the pull request.", and replace the intro paragraph and the 01 / Navigate and 03 / Verify paragraphs with the D8 copy. Keep exactly three articles, with their labels and H3s (design.md D3).
- [x] 3.4 In the same file's "Never changes your work" card, append the pull request sentence from D8 (`marketing-site`: *Read-only is framed as a design choice*).
- [x] 3.5 Replace the intro at `site/pages/docs/+Page.tsx:8` with the D8 `/docs` intro (`marketing-site`: *Landing page states the product's positioning*, scenario *Docs index intro carries the same framing*).
- [x] 3.6 Read every changed site string for British English and for any version number. The site's no-version e2e test enforces the latter.

## 4. Site E2E reconciliation

- [x] 4.1 In `site/e2e/tests/landing.spec.ts`, update the H1 assertion at L126 and the pillars H2 in the H2 list at L139-140. Keep the kicker and summary assertions at L127-129 passing unchanged.
- [x] 4.2 In `site/e2e/tests/routes.spec.ts:6`, update the `/` heading to the new H1.
- [x] 4.3 Add assertions to `site/e2e/tests/landing.spec.ts` for the new scenarios:
  - the `<title>`;
  - the meta description names the proposal and the pull request;
  - the kicker text;
  - the Navigate article names the Dashboard;
  - the Verify article names pull requests and viewed hunks;
  - the read-only card covers pull requests;
  - no "viewer" inside `main`.

  Assert the `/docs` intro scenario in the spec that already covers `/docs`.
- [x] 4.4 Prove the new assertions bite. Temporarily restore the old 03 / Verify paragraph, run `bun run site:test`, confirm the pull request assertion FAILS, then restore.

## 5. Verification

- [x] 5.1 In the fresh worktree, run `bun install` at the root and `bun install --cwd site`, because the site has its own lockfile.
- [x] 5.2 `cargo test`
- [x] 5.3 `bun test` (root; covers `npm/packaging.test.ts`)
- [x] 5.4 `bun run build`
- [x] 5.5 `bun run site:build`, then `bun run site:test` and `bun run site:test:unit`
- [x] 5.6 Manual smoke in `bun run wt:dev` (the implementer runs the app). Open the macOS About panel and confirm the new tagline, then walk the *Bundle descriptions and the About tagline carry the positioning* scenario.
- [x] 5.7 `openspec validate reframe-product-positioning --strict`
