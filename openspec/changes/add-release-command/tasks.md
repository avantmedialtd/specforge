## 1. Notes directory and workflow wiring

- [x] 1.1 Establish the `releases/` directory convention and document the `releases/vX.Y.Z.md` filename rule (tag including the leading `v`, matching `github.ref_name`).
- [x] 1.2 In `.github/workflows/release.yml`, add `actions/checkout@v6` to the `release` job (checking out the tagged ref) so the notes file is present.
- [x] 1.3 Replace the inline `body:` with `body_path: releases/${{ github.ref_name }}.md` and set `generate_release_notes: false`.
- [x] 1.4 Move the Windows-downloads / WebView2 wording out of the workflow body and into the command's generated footer template, so `release-pipeline`'s WebView2-documentation requirement is still satisfied by the file.

## 2. The /release command

- [x] 2.1 Create `.claude/commands/release.md` with frontmatter (name/description/category/tags) consistent with the `opsx` commands.
- [x] 2.2 Implement the master-only preflight: hard-fail when not on `master` or the target tag exists; require explicit confirmation on a dirty tree, `master` ahead of `origin/master`, or a non-green latest master CI run.
- [x] 2.3 Resolve the current version from the latest `v*` tag and compute the release window: `git log <lastTag>..HEAD`, plus newly-added archive dirs via `git log <lastTag>..HEAD --diff-filter=A --name-only -- openspec/changes/archive/`.
- [x] 2.4 Infer the suggested bump type from the spec-delta signal (new `specs/<capability>/` dir ⇒ ≥ minor; only polish ⇒ patch; never major) and present the type prompt (major/minor/patch/explicit) with the suggestion; decline prerelease input.
- [x] 2.5 Resolve the target version with `bun run version <type> --dry-run` and parse `vX.Y.Z -> vX.Y.Z` output.
- [x] 2.6 Synthesize hybrid notes (archived `proposal.md` titles + "why" lines as the spine; `git log` for bare commits) into the playful section template (✨ Highlights / 🔧 Improvements / 🗑 Removed / 🩹 Fixes) plus the Downloads footer (macOS Gatekeeper workaround, Windows WebView2 note, all-platform downloads, generated compare link).
- [x] 2.7 Write `releases/<tag>.md` uncommitted; present the approval gate showing the resolved version, preflight result, and fully rendered notes, with edit/cancel/proceed and no mutation before approval.
- [x] 2.8 On approval: commit the notes file, run `bun run version <type>` to tag the notes commit, and push `master --follow-tags`.
- [x] 2.9 Follow the release workflow run to publication; on success report the published release URL, on failure report the failing job and offer recovery (delete tag + re-release same version, or patch forward).

## 3. Validation

- [x] 3.1 `openspec validate add-release-command --strict` passes.
- [x] 3.2 Exercise the command's read-only phases against the current repo (resolve version, compute the v0.5.0→next window, synthesize a draft notes file) without committing, tagging, or pushing.
- [x] 3.3 Lint/format touched files and confirm `release.yml` still parses (YAML/actionlint check).
