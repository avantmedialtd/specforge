---
name: "Release"
description: Cut a release — pick the version type, write proper notes, tag, push, and follow the build to published
category: Workflow
tags: [release, workflow]
---

Cut a SpecForge release end to end: choose the version type, synthesize proper
release notes from what actually shipped, write them to a versioned file, get
explicit approval, then tag, push, and follow the build to a published GitHub
Release.

**Input**: none. The release type is chosen interactively (with a suggestion).

**Where this runs**: the **primary checkout, on `master`** — *not* a worktree.
Feature work lands on `master` via `/complete-work` first; `/release` is the
deliberate exception that runs from the main repo to ship what has landed.

---

## Steps

### 1. Preflight (master-only safety)

Run these read-only checks before anything else. **Hard-fail** (stop, report, do
nothing) on the first two; **confirm-through** (surface the condition and require
an explicit yes via AskUserQuestion) on the rest.

```bash
git rev-parse --abbrev-ref HEAD          # must be: master   → else HARD FAIL
git status --porcelain                   # non-empty = dirty  → CONFIRM
git fetch origin --quiet
git rev-list --left-right --count origin/master...master   # "ahead" > 0 → CONFIRM
gh run list --branch master --limit 1 --json conclusion,status   # not success → CONFIRM
```

- **Not on `master`** → HARD FAIL: "Releases run from the primary checkout on `master`."
- The target **tag already exists** (checked in step 4 once the version is known) → HARD FAIL.
- **Dirty tree**, **local `master` ahead of `origin/master`**, or **latest master CI not green** → show the specific condition and ask to continue (AskUserQuestion: Continue / Cancel).

### 2. Resolve the current version and the release window

```bash
LAST_TAG=$(git tag -l 'v*' --sort=-creatordate | head -1)      # e.g. v0.5.0
git log "$LAST_TAG"..HEAD --oneline                            # every commit shipping
git log "$LAST_TAG"..HEAD --diff-filter=A --name-only \
    -- openspec/changes/archive/                               # newly-archived dirs
```

For each archive directory **added** in that range, read its `proposal.md` (title
+ the **Why** lines). Those are the spine of the notes. Note which archived
changes added a new `specs/<capability>/` directory — that is the bump signal.

### 3. Choose the release type (with an inferred suggestion)

Infer a **suggested** bump, but never apply it automatically:

```
any archived change in the window added a new specs/<capability>/ dir   → suggest MINOR
otherwise (only modifications / removals / fixes / polish)              → suggest PATCH
never auto-suggest MAJOR  (pre-1.0, the 1.0 call is a human milestone)
```

Ask with **AskUserQuestion**, putting the suggested option first and labelling it
`(suggested)`, e.g. for `v0.5.0` with new capabilities:

- **Minor (suggested)** → `v0.6.0` — N changes incl. K new capabilities
- **Patch** → `v0.5.1`
- **Major** → `v1.0.0`
- *Other* → an explicit `x.y.z`

**Decline prerelease input.** If the explicit version carries a `-rc` / `-beta`
suffix, stop and explain prerelease versions aren't supported yet (v1 is final
`x.y.z` only). `bump-version.ts`'s regex would reject it anyway.

### 4. Resolve the target version

```bash
bun run version <type> --dry-run        # prints: v0.5.0 -> v0.6.0   (writes nothing)
```

Parse the right-hand side as the target tag (e.g. `v0.6.0`). Confirm the tag does
not already exist (HARD FAIL from step 1 if it does):

```bash
git rev-parse -q --verify "refs/tags/v0.6.0" && echo EXISTS
git ls-remote --tags origin "v0.6.0"
```

### 5. Synthesize the notes

Curate — do **not** dump commit subjects. Use the archived proposals as the
spine and `git log` for bare commits, rewriting imperative subjects into
user-facing product voice. Group into the sections below (omit any empty
section). Then append the Downloads footer **with the real version substituted**
and a generated compare link.

```markdown
SpecForge v0.6.0

<optional one-line theme of the release>

## ✨ Highlights
- **<Feature>** — <what it does for the user>.

## 🔧 Improvements
- <smaller enhancement / restyle / polish>.

## 🗑 Removed
- <user-visible removal>.

## 🩹 Fixes
- <bug fix>.

---

### Downloads

- **macOS** — `SpecForge_<version>_universal.dmg` (Apple Silicon + Intel).
  This build is **unsigned**: on first launch, right-click the app ▸ **Open** and
  confirm, or clear the quarantine flag:
  `xattr -dr com.apple.quarantine /Applications/SpecForge.app`.
- **Windows** — `SpecForge_<version>_x64-setup.exe` (installer, recommended —
  ensures the Microsoft Edge WebView2 runtime) or
  `SpecForge_<version>_x64-portable.exe` (single file; uses the system WebView2
  runtime, preinstalled on Windows 11 and maintained Windows 10 — use the
  installer if your machine lacks it).
- **Linux** — `.deb` or `.AppImage`.

#### Terminal UI (`specforge-tui`)

The standalone terminal client, one archive per platform — extract and run
`./specforge-tui`:

- **macOS** — `specforge-tui_<version>_macos-universal.tar.gz` (Apple Silicon +
  Intel). **Unsigned**, and a terminal binary has no right-click ▸ Open dialog,
  so clear the quarantine flag before the first run:
  `xattr -dr com.apple.quarantine specforge-tui`.
- **Linux** — `specforge-tui_<version>_linux-x64.tar.gz`.
- **Windows** — `specforge-tui_<version>_windows-x64.zip`.

#### Standalone Web Server (`specforge-serve`)

The headless web server. The quickest way to run it is npm — no download, and
**no quarantine step**, because npm installs are not flagged the way browser
downloads are:

```bash
npx @avantmedia/specforge          # or: npm install -g @avantmedia/specforge
```

Only your platform's binary is fetched. It binds `127.0.0.1:4317` by default;
`--bind 0.0.0.0` (or any other interface address) publishes it on the network,
**unauthenticated** — only do this on a network you trust.

Prefer a direct download? One archive per platform — extract and run
`./specforge-serve`:

- **macOS** — `specforge-serve_<version>_macos-universal.tar.gz` (Apple
  Silicon + Intel). **Unsigned**, and a terminal binary has no right-click ▸
  Open dialog, so clear the quarantine flag before the first run:
  `xattr -dr com.apple.quarantine specforge-serve`. (This does not apply to
  the npm install above.)
- **Linux** — `specforge-serve_<version>_linux-x64.tar.gz` or
  `specforge-serve_<version>_linux-arm64.tar.gz`. Statically linked against
  musl, so one binary covers Alpine and containers as well as current and
  older glibc distributions.
- **Windows** — `specforge-serve_<version>_windows-x64.zip`.

**Full Changelog**: https://github.com/avantmedialtd/specforge/compare/<lastTag>...v0.6.0
```

### 6. Write the notes file (no mutation yet)

Write the synthesized notes to **`releases/<tag>.md`** (the tag includes its
leading `v`, e.g. `releases/v0.6.0.md`). Do **not** `git add`/commit/tag/push
yet — this is just a file on disk so it can be reviewed and edited.

### 7. Approval gate

Show a release plan and the **fully rendered notes**, then ask with
**AskUserQuestion**: **Proceed / Edit / Cancel**.

```
Release plan
  Version:  v0.5.0 → v0.6.0   (<type>, suggested: <yes/no>)
  Ref:      master @ <sha>    <preflight result inline>
  Window:   <N> commits · <K> archived changes
  Notes:    releases/v0.6.0.md
<full rendered notes markdown>

This commits the notes, tags v0.6.0, and triggers the public release
build (~20 min). Proceed?
```

- **Edit** → take the user's wording changes (or let them edit the file), rewrite `releases/<tag>.md`, and re-present the gate.
- **Cancel** → stop. Leave the uncommitted notes file on disk (a later `/release` run can reuse it). Nothing was tagged or pushed.
- **Proceed** → step 8.

### 8. Write the site version, commit, tag, push (only after approval)

Set `RELEASE_VERSION` in **`site/src/site-config.ts`** to the target version
**without** its leading `v` (e.g. `0.6.0`), set `modified` in
**`site/pages/changelog/+documentProps.ts`** to today's date in UTC, then commit
both together with the notes file:

```bash
# site/src/site-config.ts:                 export const RELEASE_VERSION = '0.6.0';
# site/pages/changelog/+documentProps.ts:  modified: '2026-09-04',
git add releases/<tag>.md site/src/site-config.ts site/pages/changelog/+documentProps.ts
git commit -m "Release <tag>"
bun run version <type>                   # creates the annotated tag on this commit
git push origin master --follow-tags     # sends the notes commit and the tag together
```

**Use the UTC date, not the local one.** The site's discovery plugin compares
`modified` against `new Date().toISOString()` and throws on a future date, so a
date taken from a local calendar already past midnight while CI is still on the
previous UTC day fails the site build outright. `date -u +%F` gives the right
value.

The changelog page renders this release's notes, so a `modified` left at the
previous release's date would misreport the page's freshness in `sitemap.xml`
from here on. The site build requires the field and never derives it — there is
no fallback that would notice.

Four things make this ordering load-bearing:

- **`bun run version` cannot write the constant.** It tags a commit that already
  exists, so it runs *after* the commit the constant must be part of. It edits
  no files by design — do not change that.
- **The push must come from this checkout, not from CI.** GitHub does not create
  workflow runs from pushes authenticated with `GITHUB_TOKEN`, so a version
  commit made by a workflow would never trigger `site.yml`, and the site would
  silently keep advertising the previous release.
- **`site/src/site-config.ts` is what triggers the site deploy.** It matches
  `site.yml`'s `site/**` path filter, so this one push both tags the release and
  republishes the marketing site.
- **The notes file is now site content, not just the GitHub release body.** The
  `/changelog` page renders it, so the notes and the constant that selects them
  must stay in one commit. `site.yml` also watches `releases/**`, which covers
  the separate case of correcting a note after its release.

`release.yml`'s `check-site-version` job asserts the constant equals the tag and
runs before the platform builds, so a forgotten bump fails in seconds. The site
deploys in ~2 minutes while the assets take ~20, so its download links 404 for
roughly eighteen minutes after each release — this is expected and accepted.

### 9. Follow the build to published — or recover

The release workflow's publish job depends on all three platform builds, so a
failed build leaves the tag pushed but **no** release published. Follow the run
(the repo convention after any push):

```bash
gh run watch "$(gh run list --workflow release.yml --branch <tag> --limit 1 --json databaseId -q '.[0].databaseId')" --exit-status
```

- **Success** → report the published release URL (`gh release view <tag> --web`).
- **Failure** → report the failing job (`gh run view <id> --log-failed`) and offer recovery via AskUserQuestion:
  1. **Delete the tag and re-release the same version** — `git push origin --delete <tag>` + `git tag -d <tag>`, fix, re-run `/release`.
  2. **Ship a follow-up patch** — fix forward and release `<tag+patch>`.

---

## Guardrails

- **Nothing mutates before the approval gate.** Up to step 7 the only side effect is writing `releases/<tag>.md` on disk; all git/`gh` calls are read-only.
- **The site version is part of the release commit, not a follow-up.** `site/src/site-config.ts` carries the version every download URL on the marketing site is built from. Bump it in step 8 or `release.yml`'s `check-site-version` job fails the release before any build starts.
- **So is the changelog page's date.** `site/pages/changelog/+documentProps.ts` carries a `modified` the site build requires and never derives; use the **UTC** date, since a future one fails that build. Nothing checks it the way `check-site-version` checks the version — a stale date publishes a page that quietly misreports its own freshness.
- **Runs on `master` in the primary checkout** — never a worktree branch.
- **Curate, don't dump.** The value over GitHub's auto-notes is editorial: user-facing voice, grouped sections, real highlights.
- **Final releases only (v1).** Decline `-rc` / `-beta`. This is now an editorial policy rather than a technical limit: the workflow derives `prerelease` and `make_latest` from the tag, and npm publishes a prerelease to the `next` dist-tag, so the pipeline handles them correctly. But an RC's notes want different framing (what to test, which install command, why it exists), and `bun run version` rejects prerelease strings by design. Cut a prerelease by hand — write `releases/<tag>.md`, then tag directly.
- **The notes file name carries the leading `v`** so it matches `github.ref_name` and the workflow's `body_path: releases/${{ github.ref_name }}.md`.
- **Substitute the real version** into the Downloads footer and the compare link — no `<version>` placeholders in the published body.
