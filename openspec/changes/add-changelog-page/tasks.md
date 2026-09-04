## 1. Build-time release-note reader

- [ ] 1.1 Add `marked` to `site/package.json` **devDependencies** and update `site/bun.lock` in the same commit — CI installs with `--frozen-lockfile`, so a package.json change without the lockfile fails before typecheck (`changelog-page`: *Markdown conversion never reaches the visitor*)
- [ ] 1.2 Create `site/build/releaseNotes.ts` (a new first-party build directory — **not** `site/site-kit/`, which is a vendored fork under a `diff -r` drift check) exporting a reader that resolves the notes directory relative to the site root and reads with `node:fs`, never through Vite's module graph (design: *Read the notes with node:fs*)
- [ ] 1.3 Implement the Downloads cut: return only the content preceding each note's `### Downloads` heading (`changelog-page`: *Release notes are adapted for the web, not shipped as authored*)
- [ ] 1.4 Throw a build-failing error naming the file when a note the page renders carries no `### Downloads` heading (`changelog-page`: *The cut contract is asserted, not assumed*)
- [ ] 1.5 Parse each note's identity from the cut body: the bare first line as the version heading, and the paragraph that follows as the standfirst — tolerating a note that has no standfirst, e.g. `releases/v0.0.2.md` (`changelog-page`: *A note without a standfirst still renders*)
- [ ] 1.6 Throw a build-failing error naming the missing file when the note for the advertised `RELEASE_VERSION` does not exist (`changelog-page`: *A missing notes file fails the build*)
- [ ] 1.7 Sort releases newest-first by semantic version and exclude prerelease tags such as `v0.19.0-rc.1` (`changelog-page`: *Prereleases are omitted*)
- [ ] 1.8 Configure `marked` to demote the notes' headings one level, and to emit version-namespaced, emoji-stripped heading ids that are unique within the document and stable across builds (`changelog-page`: *Generated heading ids are unique across releases*)
- [ ] 1.9 Derive each release's date from its git tag or note metadata rather than file mtime, so the displayed dates survive a fresh clone

## 2. The changelog page

- [ ] 2.1 Create `site/pages/changelog/+data.ts` calling the reader — a server-only Vike hook, so the parser stays out of the client graph (`changelog-page`: *No parser is served to the browser*)
- [ ] 2.2 Create `site/pages/changelog/+documentProps.ts` with `title`, `description`, `path: '/changelog'` and an authored `modified` ISO date — the discovery plugin fails the build without one (`marketing-site`: *Adding a page without a date fails the build*)
- [ ] 2.3 Create `site/pages/changelog/+Page.tsx` rendering a single `<h1>`, then the current release's heading, date, standfirst and converted body via `dangerouslySetInnerHTML` (`changelog-page`: *The changelog route renders the current release*)
- [ ] 2.4 Render the earlier-releases list below it — version, date, standfirst, each linking to its GitHub Release page — with no section headings or bullets from those notes (`changelog-page`: *Earlier releases are listed in condensed form*)
- [ ] 2.5 Confirm the rendered body contains no second `<h1>` and that the note's version line does not also appear in the body (`changelog-page`: *The version line becomes a heading*)

## 3. Styling

- [ ] 3.1 Add a `.prose-notes` block to `site/src/styles.css` inside `@layer components`, sibling to `.prose-docs` — an un-layered rule would outrank every Tailwind utility (`changelog-page`: *Rendered notes carry the site's own typography*)
- [ ] 3.2 Style generated headings, `strong`, and `hr` from the existing `:root` design tokens; `a`, `code`, `pre` and `[id]` already inherit from `@layer base` and need no new rules
- [ ] 3.3 Normalise list spacing so notes authored with loose markup (v0.14.0–v0.18.0) and tight markup render identically (`changelog-page`: *List spacing does not vary by release era*)
- [ ] 3.4 Verify both colour schemes, since the tokens are re-tuned under `prefers-color-scheme: dark`

## 4. Navigation and site wiring

- [ ] 4.1 Add a `/changelog` link to the header nav in `site/src/Layout.tsx`, beside `Docs` — the primary nav carries no link-count assertion, so this costs no test change
- [ ] 4.2 Do **not** add the route to `DOCS_NAV` in `site/src/site-config.ts`: it would break `layout.spec.ts`'s exact docs-sidebar count of 8, and the changelog is not a docs page
- [ ] 4.3 Add `releases/**` to the `push` and `pull_request` path filters in `.github/workflows/site.yml`, so a corrected note redeploys the site on its own (`marketing-site`: *A corrected release note redeploys the site*)

## 5. Release command

- [ ] 5.1 Update `.claude/commands/release.md` step 8 to write the changelog page's `modified` date and `git add` it alongside `releases/<tag>.md` and `site/src/site-config.ts` (`release-command`: *The release commit refreshes the changelog page's date*)
- [ ] 5.2 Record in that step that the date is compared against UTC and that a future date fails the site build, so it must not be taken from a local calendar running ahead of UTC

## 6. Tests

- [ ] 6.1 Add `/changelog` to the route list in `site/e2e/tests/seo.spec.ts` — it asserts exact set-equality against the derived sitemap and is the one mandatory test edit (`marketing-site`: *Every route is in the sitemap*)
- [ ] 6.2 Add a structural changelog spec: status 200, exactly one `<h1>`, the version imported from `site/src/site-config.ts` present on the page, and at least one rendered section — no assertion on wording (`changelog-page`: *A new release does not break the suite*)
- [ ] 6.3 Leave `/changelog` out of `downloads.spec.ts`'s ROUTES loop and record the reason in a comment: its body-scanning guards exist for site-authored copy, and `releases/v0.19.0-rc.1.md` already contains a string its pinned-install regex bans (`changelog-page`: *Copy guards do not police authored release notes*)
- [ ] 6.4 Add `/changelog` to the route lists in `routes.spec.ts` and `layout.spec.ts` for coverage, declaring its expected `<h1>` text
- [ ] 6.5 Add a unit-level test of the reader covering the cut, a note with no standfirst, prerelease exclusion, and the two build-failing errors (missing file, missing cut point)

## 7. Verification

- [ ] 7.1 `bun install --cwd site` then `bun run site:build` — confirm it succeeds and that `site/dist/client/sitemap.xml` lists exactly ten routes
- [ ] 7.2 `bun run --cwd site typecheck` and `bun run --cwd site check:spelling`
- [ ] 7.3 `bun run site:test` — the full Playwright suite, which gates the deploy job
- [ ] 7.4 Grep the built client bundle to confirm no `marked` code shipped, and confirm `site/package.json` runtime dependencies are unchanged (`changelog-page`: *The runtime dependency set is unchanged*)
- [ ] 7.5 Run `bun run site:dev` and walk the page in a browser: current release renders with styled headings and consistent bullet spacing, earlier releases are condensed, no Downloads content or artefact filenames appear, and heading anchors scroll clear of the sticky header — in both colour schemes (the implementer runs this, never the user)
- [ ] 7.6 Temporarily point `RELEASE_VERSION` at a nonexistent version and confirm the build fails naming the missing file; revert
- [ ] 7.7 Confirm the desktop app is untouched: `bun run build` and `cargo test` behave exactly as before this change (the site is outside every workspace glob, so both should be unaffected)
