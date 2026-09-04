# Tasks

## 1. Version wire

- [ ] 1.1 Add a `RELEASE_VERSION` constant and an asset-URL helper to `site/src/site-config.ts`, deriving each artefact's URL from the version and the documented filename scheme; document that `/release` owns the constant (`marketing-site`: *Downloads name the current release and link its assets*)
- [ ] 1.2 Add a `check-site-version` job to `.github/workflows/release.yml` that compares `RELEASE_VERSION` against `github.ref_name` with the leading `v` stripped, fails naming both values, and is a `needs:` prerequisite of every platform build job (`release-pipeline`: *Site Version Matches Tag*)
- [ ] 1.3 Extend step 8 of `.claude/commands/release.md` to write the resolved version into `site/src/site-config.ts` and `git add` it alongside the notes file, with a note that the bump script cannot do this because it runs after the commit (`release-command`: *Commit, Tag, And Push On Approval*)
- [ ] 1.4 Record in `.claude/commands/release.md` that the push must come from the developer's checkout, because a CI push carries the workflow token and would not trigger `site.yml` (`release-command`: *Commit, Tag, And Push On Approval*)

## 2. Hero restructure

- [ ] 2.1 Delete the `.hero-platforms` strip and its four inert spans from `site/pages/index/+Page.tsx` (`marketing-site`: *The download block sits above the fold*)
- [ ] 2.2 Replace the `#downloads` anchor button in `.hero-actions` with a real download control, deleting the "Get SpecForge" call to action (`marketing-site`: *The download block sits above the fold*)
- [ ] 2.3 Shorten the hero summary and fold `.hero-proof` into it, and cap the `.product-stage` column, so the download control and the npm command both clear a 1440x900 fold (`marketing-site`: *The download block sits above the fold*)
- [ ] 2.4 Keep the `npx` command as the co-equal second route in the hero and remove its two other renderings — the `.hero-command` block it replaces and the `ProductSurface` "Browser" meta line (`marketing-site`: *Downloads name the current release and link its assets*)
- [ ] 2.5 Remove `.hero-platforms` rules from `site/src/styles.css`, including the responsive overrides in the mobile block, and add styles for the new download control

## 3. Full artefact list

- [ ] 3.1 Replace the three inert `Download` cards in `site/pages/index/+Page.tsx` with a list in which every one of the twelve artefacts links its release asset (`marketing-site`: *Downloads name the current release and link its assets*)
- [ ] 3.2 Style downloading controls distinctly from navigating links, so nothing that only navigates carries download styling (`marketing-site`: *Downloads name the current release and link its assets*)
- [ ] 3.3 Keep the unsigned-build caveat and the network-bind warning with the list, preserving their links to `/docs/troubleshooting` and `/docs/web-ui`
- [ ] 3.4 Remove the dead `.download-card`, `.download-grid` and `.download-formats` rules from `site/src/styles.css`

## 4. Platform detection

- [ ] 4.1 Add a client-side platform resolver reading only `navigator`, returning the matching artefact or nothing, under `site/src/components/` (`marketing-site`: *The primary download follows the visitor's platform*)
- [ ] 4.2 Render the platform-neutral control in HTML and relabel and retarget it on hydration, so the server and client markup agree on first paint (`marketing-site`: *The primary download follows the visitor's platform*)
- [ ] 4.3 Confirm the resolver issues no request and reads no cookie, leaving *No cookies and no analytics* intact (`marketing-site`: *The primary download follows the visitor's platform*)

## 5. Tests

- [ ] 5.1 Replace the version-string and `/releases/download/` prohibitions in `site/e2e/tests/downloads.spec.ts` with assertions that each advertised artefact links an asset URL and that the offered version is stated
- [ ] 5.2 Add a test asserting no landing-page call to action targets a same-page anchor, and that no element styled as a control lacks a link or handler
- [ ] 5.3 Add a viewport test asserting a download control and the npm command are both above the fold at 1440x900
- [ ] 5.4 Add a scripting-disabled test asserting the neutral control still resolves and every platform's asset stays reachable
- [ ] 5.5 Confirm `site/e2e/tests/cookies.spec.ts` still passes unchanged — it is the guard proving detection calls nobody
- [ ] 5.6 Update `site/e2e/tests/landing.spec.ts` for the removed hero strip and changed call to action

## 6. Verification

- [ ] 6.1 `bun run site:build` — succeeds, and `site/dist/client/index.html` contains the versioned asset URLs
- [ ] 6.2 `bun run site:test` — all Playwright specs green in both the desktop and mobile projects
- [ ] 6.3 `node site/scripts/check-uk-spelling.mjs` — clean over `pages/` and `src/`
- [ ] 6.4 `bun test` at the repository root — still 22 files, proving `bunfig.toml` still excludes the site's specs
- [ ] 6.5 Serve the built site locally and walk the change's scenarios in a browser at 1440x900: download without scrolling, every artefact link resolves, the label follows the detected platform, and no request leaves the origin
- [ ] 6.6 Dry-run the release guard by running the `check-site-version` comparison against a deliberately mismatched tag, confirming it fails and names both values
- [ ] 6.7 `openspec validate versioned-downloads-above-fold --strict`
