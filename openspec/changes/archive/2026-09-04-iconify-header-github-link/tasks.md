## 1. The mark

- [x] 1.1 Obtain GitHub's official `mark-github` path (16×16 `viewBox`) from its
  published source rather than reproducing it from memory — a dropped subpath or a
  wrong fill-rule is invisible at 16px. Note the source in the component's comment.
- [x] 1.2 Add a local `GitHubMark` function to `site/src/Layout.tsx`, directly after
  `SpecForgeMark`, rendering that path in a 16×16 `viewBox` with
  `fill="currentColor"`, `aria-hidden="true"`, explicit `width`/`height`, and
  `className="block"` so the SVG contributes no inline strut
  (`marketing-site`: *The header marks its off-site link rather than labelling it*).
- [x] 1.3 Do not create `site/src/components/icons.tsx`, and do not add an icon
  package to `site/package.json` — the design records why both were rejected.

## 2. The navigation link

- [x] 2.1 In `site/src/Layout.tsx`, replace the `GitHub` text inside the
  `href={REPO_URL}` anchor with `<GitHubMark />`, keeping the anchor's `href`, its
  position between the `/docs` and `/#downloads` links, and its
  `text-[var(--text-muted)] no-underline hover:text-[var(--text)]` classes exactly
  as they are (`marketing-site`: *The header marks its off-site link rather than
  labelling it*).
- [x] 2.2 Add `aria-label="GitHub"` to that anchor so its accessible name is
  unchanged from the word it replaces
  (`marketing-site`: *A control reduced to a glyph keeps its name and its target*).
- [x] 2.3 Make the anchor `inline-flex` and give it `p-2 -m-2`, so the activation
  target reaches 32×32 while its layout contribution stays the mark's own 16px
  (`marketing-site`: *A control reduced to a glyph keeps its name and its target*).
- [x] 2.4 Comment the load-bearing choices in this file's established idiom — why
  `inline-flex` rather than the inherited inline formatting context (the strut
  already documented on the brand anchor above), why the negative margin offsets the
  padding, and why the mark is sized down rather than the `--text-muted` token
  being brightened.

## 3. Measure and tune

- [x] 3.1 Start the site yourself with `bun run site:dev` — never ask the user to
  run it — and confirm the rendered mark matches GitHub's reference glyph at 16px.
- [x] 3.2 Compare the mark's optical weight against `Docs` in the same row, under
  both `prefers-color-scheme: light` and `dark` (the site themes solely through that
  media query at `site/src/styles.css:91`; there is no toggle to click). Fall back to
  18px if 16px reads small, and change nothing about the colour token either way.
- [x] 3.3 Measure the header row's intrinsic width at a 390px viewport before and
  after the change and record both figures in the commit message — the header's own
  comments cite measured pixels, so this change should too rather than asserting an
  estimated saving.
- [x] 3.4 Confirm in the browser that the anchor's bounding box is ≥ 24px in each
  dimension and that its bounding box plus horizontal margins does not exceed the
  glyph's own width.

## 4. Tests

- [x] 4.1 Extend the header test in `site/e2e/tests/routes.spec.ts` (currently
  asserting only the `href` at line 46) to assert the repository link's **accessible
  name** is `GitHub`, so a dropped `aria-label` fails instead of passing silently
  (`marketing-site`: *A control reduced to a glyph keeps its name and its target*).
- [x] 4.2 Add a guard to `site/e2e/tests/layout.spec.ts`, beside the existing
  computed-style guards, asserting the repository link's activation target measures
  at least 24×24 CSS px and that its bounding box plus horizontal margins does not
  exceed the glyph's width
  (`marketing-site`: *A control reduced to a glyph keeps its name and its target*).
- [x] 4.3 Assert the mark itself is hidden from assistive technology, so the link is
  announced once rather than twice
  (`marketing-site`: *A control reduced to a glyph keeps its name and its target*).
- [x] 4.4 Confirm the existing narrow-viewport overflow tests at 320/360/375px still
  pass unchanged — they should improve, not regress.

## 5. Verification

Mirrors `.github/workflows/site.yml`, in its order. Every command runs with the
working directory set to `site/`; the root scripts already do that.

- [x] 5.1 `bun run --cwd site typecheck`
- [x] 5.2 `bun run --cwd site check:spelling`
- [x] 5.3 `bun run site:build`
- [x] 5.4 `bun run site:test`
- [x] 5.5 Manual smoke in `bun run site:dev`, run by the implementer, walking each
  scenario in the delta spec: the repository link renders as a mark with no visible
  label; its resting colour matches the `Docs` link and hovers the same way; the
  navigation still offers docs, repository and download in that order at the same
  targets; the link is announced as `GitHub`; the mark is hidden from assistive
  technology; the target measures ≥ 24×24; and no page scrolls sideways at 320, 360,
  375 or 390px.
- [x] 5.6 Confirm no Rust crate, no `src/types.ts` entry and no `site/bun.lock` line
  changed — this change is confined to `site/src/Layout.tsx`, two files under
  `site/e2e/tests/`, and this change's own artifacts.
