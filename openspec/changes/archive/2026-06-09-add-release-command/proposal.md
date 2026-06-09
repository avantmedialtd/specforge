# Add a guided /release command

## Why

Cutting a release today is two manual steps with throwaway notes: run `bun run version <type>`, push the tag, and the release workflow publishes a GitHub Release whose body is a static Windows-downloads blurb plus GitHub's raw auto-generated commit list. Nobody ever writes *what actually shipped*, so every release reads the same and tells users nothing. We want a single guided command that picks the version and authors proper, curated release notes — and SpecForge already holds the perfect raw material, since each archived OpenSpec change is a dated, titled record of what was built.

## What Changes

- Add a `/release` project command (`.claude/commands/release.md`) that runs on the primary checkout / `master` and drives a release end to end: pick type → synthesize notes → approval gate → commit + tag + push → follow the build to *published*, or offer recovery on failure.
- The release **type** is chosen via a prompt with a **suggested** bump inferred from what shipped: a change that introduced a new capability spec ⇒ feature ⇒ at least `minor`; a window of only fixes/polish ⇒ `patch`. `major` is never auto-suggested (pre-1.0 the 1.0 call is a human milestone), though the user can still pick it.
- Release notes are **synthesized from a hybrid source** — the proposals of OpenSpec changes archived since the last tag as the spine, with `git log <lastTag>..HEAD` filling in bare commits that never had a change directory — and curated into playful, user-facing sections (✨ Highlights / 🔧 Improvements / 🗑 Removed / 🩹 Fixes).
- Notes are written to a versioned, in-repo file at **`releases/vX.Y.Z.md`**, shown in full for approval, then committed and tagged.
- The notes footer documents per-platform downloads and install caveats, including the macOS **Gatekeeper / unsigned-app** workaround (new) alongside the existing Windows WebView2 note, plus a generated Full-Changelog compare link.
- `release.yml` sources the GitHub Release body from `releases/<tag>.md` (via `body_path`) instead of the inline static body, adds the `actions/checkout` the publish job currently lacks, and turns off GitHub's auto-generated notes.
- **Non-goal (v1):** prerelease versions (`-rc` / `-beta`). The command handles final `x.y.z` only, matching `bump-version.ts`'s strict regex and the pipeline's fixed `make_latest` / `prerelease` flags.

## Capabilities

### New Capabilities

- `release-command`: the interactive, guided release command — version-type selection with an inferred suggestion, hybrid release-note synthesis, the versioned `releases/<tag>.md` notes file, the pre-push approval gate, the master-only safety preflight, and following the build to a published release or a recovery offer when the build fails.

### Modified Capabilities

- `release-pipeline`: the publish job sources the GitHub Release body from the versioned `releases/<tag>.md` file committed at the tagged ref, rather than an inline static body plus GitHub-auto-generated notes.

## Impact

- **New file:** `.claude/commands/release.md` (the command definition).
- **New directory convention:** `releases/vX.Y.Z.md` — per-version release notes, committed to the repo.
- **Modified:** `.github/workflows/release.yml` — add `actions/checkout@v6` to the `release` job; replace the inline `body:` with `body_path: releases/${{ github.ref_name }}.md`; set `generate_release_notes: false`.
- **Reused unchanged:** `scripts/bump-version.ts` (`--dry-run` to resolve the next version for the filename, then a real run to create the tag on the notes commit).
- No Rust or TypeScript source changes; no runtime application behavior change.
