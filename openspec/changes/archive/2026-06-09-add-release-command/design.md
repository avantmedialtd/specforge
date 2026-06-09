## Context

Releases are tag-driven. `scripts/bump-version.ts` derives the current version from the latest `v*` tag, computes the next one, and creates an annotated tag locally (no file edits, no push). Pushing the tag triggers `.github/workflows/release.yml`, which stamps the version, builds Linux/Windows/macOS bundles, and — in a `release` job that depends on all three — publishes a GitHub Release via `softprops/action-gh-release`. That action's body is a hard-coded Windows-downloads paragraph, and `generate_release_notes: true` appends GitHub's raw commit/PR list. The `release-pipeline` capability spec governs this workflow.

The result: every release body is interchangeable boilerplate. Meanwhile SpecForge dogfoods OpenSpec, so the dated `openspec/changes/archive/<date>-<id>/` directories are a precise, titled ledger of what shipped between any two tags — including which ones introduced a brand-new `specs/<capability>/` directory.

## Goals / Non-Goals

**Goals:**
- One guided `/release` command that takes a release from "master is ready" to "published GitHub Release with proper notes."
- Curated, user-facing notes synthesized from the archived changes (+ git log) in the release window.
- A human approval gate before anything irreversible (the tag push triggers a public build + publish).
- Reuse the existing tag-driven machinery; minimal, surgical change to `release.yml`.

**Non-Goals:**
- Prerelease versions (`-rc` / `-beta`) — `bump-version.ts`'s regex and the pipeline's fixed flags don't support them; deferred to a later change.
- Code signing / notarization — the pipeline is explicitly unsigned; out of scope.
- Changing how bundles are built, named, or which platforms ship.
- An accumulating `CHANGELOG.md` — per-version files only (a later change could add one).

## Decisions

### Notes live in the repo; CI renders them (`body_path`)
The command writes `releases/vX.Y.Z.md`, commits it, then tags that commit. `release.yml`'s publish step reads it via `body_path: releases/${{ github.ref_name }}.md` and sets `generate_release_notes: false`. Rationale: notes become a versioned, reviewable artifact tied to the exact released commit, with no wait-and-edit race against the ~20-minute build. The file keeps the `v` prefix (`v0.6.0.md`) so it matches `github.ref_name` exactly — no shell munging in CI.

**Gotcha addressed:** the `release` job today has *no* `actions/checkout` (it only downloads build artifacts), so `body_path` would resolve to nothing. The change adds `actions/checkout@v6` to that job so the tagged commit's notes file is present.

### Mutation is deferred until after the approval gate
Before the gate, the command only *writes the notes file to disk* (uncommitted) and runs read-only git/gh checks. Nothing touches git history. On approval it commits the notes, runs `bun run version <type>` to tag the notes commit, and pushes `master --follow-tags`. A cancel leaves an uncommitted `releases/vX.Y.Z.md` on disk, which a re-run can reuse. This keeps the pre-push phase freely iterable (notes can be re-drafted any number of times) and the irreversible push behind a single confirmation.

### Version resolved via `--dry-run`, then the real bump tags the notes commit
The notes filename needs the next version *before* tagging. `bun run version <type> --dry-run` prints `vX.Y.Z -> vX.Y+1.Z` and writes nothing; the command parses the target. After committing the notes it runs `bun run version <type>` for real, which tags HEAD — now the notes commit. No change to `bump-version.ts`.

### Bump type is suggested from the spec delta, never auto-applied
For each change archived since the last tag, the presence of a new `specs/<capability>/` directory is the signal that a *capability* (not just a fix) shipped ⇒ suggest `minor`. A window of only modifications/removals/polish ⇒ suggest `patch`. The suggestion only pre-labels the recommended option in the type prompt; the user always sees Major/Minor/Patch/explicit. `major` is never auto-suggested pre-1.0.

### Hybrid notes source, curated by the agent
The archived changes' `proposal.md` titles + "why" lines are the spine (they're already user-facing summaries); `git log <lastTag>..HEAD` covers bare commits with no change directory (e.g. formatting/clippy). The agent groups them into ✨ Highlights / 🔧 Improvements / 🗑 Removed / 🩹 Fixes, rewriting commit-imperative into product voice, and appends a templated Downloads footer. The footer documents all three platforms and their caveats — Windows WebView2 (existing pipeline requirement) and the macOS Gatekeeper/unsigned workaround (new) — plus a generated `compare/<old>...<new>` link (since GitHub auto-notes are now off).

### `/release` is a master-only command, by design
The repo does all feature work in worktrees via `/complete-work`; `/release` is the deliberate exception that runs on the primary checkout after work has landed. Preflight: hard-fail if not on `master` or if the target tag already exists; confirm-through if the tree is dirty, `master` is ahead of `origin/master`, or the latest master CI run isn't green. The push uses `--follow-tags` to send the notes commit and the tag together.

### Terminal states: Published or Build-failed-with-recovery
Because the `release` job is `needs: [all builds]`, a failed build means the tag is pushed but *no* Release is published — a clean, single failure state. The command follows the workflow run (the repo's standard post-push monitoring) and ends at either "Published ✓ <url>" or a recovery offer: delete the tag (local + origin) and re-release the same version, or fix forward as the next patch.

## Risks / Trade-offs

- **Notes commit noise.** Each release adds a "release notes for vX.Y.Z" commit before the tag. Accepted: the notes are genuinely part of the release and worth versioning.
- **Disabling `generate_release_notes`** drops GitHub's auto compare-link and contributor list. Mitigated by generating the compare link in the footer; contributors are moot for an effectively solo repo.
- **Suggestion can be wrong** (a feature without a spec delta, or a spec-only refactor). Mitigated: it is only a suggestion; the user always confirms the type explicitly.
- **macOS caveat accuracy.** The workaround text (`xattr -dr com.apple.quarantine` / right-click ▸ Open) must stay correct as long as the build is unsigned; it is tied to the pipeline's existing unsigned-artifacts requirement.
- **Window detection.** "Archived since the last tag" is computed from `git log <lastTag>..HEAD --diff-filter=A -- openspec/changes/archive/`, i.e. directories *added* in the commit range — precise, and independent of archive directory dates.
