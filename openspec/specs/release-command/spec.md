# release-command Specification

## Purpose

Defines the `/release` slash command (`.claude/commands/release.md`) that cuts a SpecForge release from the primary checkout on `master`: the safety preflight, the release-type prompt with its inferred suggestion, the notes synthesized from the OpenSpec changes archived since the last `v*` tag and written to `releases/<tag>.md`, the approval gate that precedes every repository mutation, and the commit-tag-push that follows it. Scope ends at the pushed tag — building and publishing the platform artifacts from that tag belongs to `release-pipeline`, which this command only follows in order to report the published release or the failing job.
## Requirements
### Requirement: Guided Release Command

The project SHALL provide a `/release` command that drives a release end to end from the primary checkout: selecting the version type, synthesizing release notes, presenting an approval gate, and — only on approval — committing the notes, tagging, pushing, and following the build to publication.

#### Scenario: Invoking the command starts a guided release

- **WHEN** the user invokes `/release` with the working tree on `master`
- **THEN** the command resolves the current version from the latest `v*` tag
- **AND** proceeds to prompt for the release type and synthesize release notes for the changes since that tag

### Requirement: Master-Only Safety Preflight

The command SHALL run only against the primary checkout on `master` and SHALL run a preflight before any irreversible action. It SHALL hard-fail when the current branch is not `master` or when the target tag already exists. It SHALL require explicit confirmation when the working tree is dirty, when local `master` is ahead of `origin/master`, or when the latest CI run on `master` is not green.

#### Scenario: Refuses to release off master

- **WHEN** the user invokes `/release` while on a branch other than `master` (for example a worktree branch)
- **THEN** the command stops without tagging or pushing
- **AND** reports that releases run from the primary checkout on `master`

#### Scenario: Refuses a version whose tag already exists

- **WHEN** the resolved target version's tag already exists locally or on the remote
- **THEN** the command stops without creating a tag
- **AND** reports the conflicting tag

#### Scenario: Confirms through softer smells

- **WHEN** the working tree is dirty, or `master` is ahead of `origin/master`, or the latest master CI run is not green
- **THEN** the command surfaces the specific condition and requires explicit confirmation before continuing

### Requirement: Release Type Selection With Inferred Suggestion

The command SHALL prompt the user to choose the release type (major, minor, patch, or an explicit `x.y.z`) and SHALL pre-label a suggested type inferred from what shipped since the last tag: a window containing any change that introduced a new capability spec directory SHALL suggest at least `minor`; a window of only modifications, removals, or polish SHALL suggest `patch`. The command SHALL NOT auto-suggest `major`. The user's explicit choice always governs.

#### Scenario: A new capability suggests minor

- **WHEN** at least one change archived since the last tag added a new `specs/<capability>/` directory
- **THEN** the type prompt pre-labels `minor` as the suggested type
- **AND** still offers major, patch, and an explicit version

#### Scenario: A fixes-only window suggests patch

- **WHEN** no change archived since the last tag added a new capability spec, and the window is only fixes or polish
- **THEN** the type prompt pre-labels `patch` as the suggested type

#### Scenario: Major is never auto-suggested

- **WHEN** the command computes a suggested type
- **THEN** the suggestion is never `major`
- **AND** the user may still select `major` explicitly

### Requirement: Hybrid Release-Note Synthesis

The command SHALL synthesize release notes from a hybrid source: the `proposal.md` of each OpenSpec change archived since the last tag forms the spine, and `git log <lastTag>..HEAD` covers commits with no corresponding archived change. The notes SHALL be curated into user-facing sections rather than reproducing raw commit subjects.

#### Scenario: Archived changes drive the notes

- **WHEN** changes were archived since the last tag
- **THEN** the synthesized notes describe those changes in user-facing language grouped into sections (such as Highlights, Improvements, Removed, Fixes)

#### Scenario: Bare commits are still represented

- **WHEN** the release window contains commits that have no corresponding archived change directory
- **THEN** those commits are still reflected in the notes (for example under Fixes or an internal grouping), not silently dropped

### Requirement: Versioned Notes File

The command SHALL write the synthesized notes to `releases/<tag>.md`, where `<tag>` is the target tag including its leading `v` (for example `releases/v0.6.0.md`), so the file name matches the tag exactly.

#### Scenario: Notes written to a per-version file

- **WHEN** the command synthesizes notes for target version `0.6.0`
- **THEN** it writes them to `releases/v0.6.0.md`

### Requirement: Notes Footer Documents Downloads And Caveats

The synthesized notes SHALL include a Downloads footer documenting the macOS, Windows, and Linux artifacts and their install caveats. The footer SHALL state the macOS Gatekeeper workaround for the unsigned build and SHALL state the Windows portable build's WebView2 prerequisite. The footer SHALL also document the npm install channel for the standalone web server, naming the published package, and SHALL state that an npm install requires no quarantine-clearing step — the workaround documented for the downloaded archives does not apply there. The notes SHALL include a Full-Changelog link comparing the previous tag to the new tag.

#### Scenario: macOS unsigned-app workaround is documented

- **WHEN** the command synthesizes a release's notes
- **THEN** the Downloads footer documents how to open the unsigned macOS build (for example a right-click ▸ Open, or clearing the quarantine attribute)

#### Scenario: Windows WebView2 prerequisite is documented

- **WHEN** the command synthesizes a release's notes
- **THEN** the footer states that the portable Windows build requires the system WebView2 runtime and that the installer is the alternative

#### Scenario: npm channel is documented for the web server

- **WHEN** the command synthesizes a release's notes
- **THEN** the footer documents installing the standalone web server from npm and names the published package

#### Scenario: npm install is stated to need no quarantine step

- **WHEN** the footer documents the npm channel alongside the macOS archive caveat
- **THEN** it states that the quarantine-clearing step applies to the downloaded archive and not to an npm install

#### Scenario: Full-Changelog link is generated

- **WHEN** the command synthesizes notes for a release following a previous tag
- **THEN** the notes include a compare link from the previous tag to the new tag

### Requirement: Approval Gate Before Any Mutation

The command SHALL defer all repository mutation until after an explicit approval gate. Before approval it SHALL only write the notes file to disk and run read-only checks; it SHALL NOT commit, tag, or push. The gate SHALL present the resolved version, the preflight result, and the fully rendered notes, and SHALL allow the user to edit the notes and re-present before approving.

#### Scenario: Nothing is committed before approval

- **WHEN** the command reaches the approval gate
- **THEN** no commit, tag, or push has occurred
- **AND** the notes exist only as an uncommitted file on disk

#### Scenario: Editing re-presents the gate

- **WHEN** the user asks to change the notes at the gate
- **THEN** the command rewrites `releases/<tag>.md` and presents the gate again with the updated rendered notes

#### Scenario: Cancelling leaves the notes file resumable

- **WHEN** the user cancels at the gate
- **THEN** no tag or push has occurred
- **AND** the uncommitted notes file remains on disk for a later run to reuse

### Requirement: Commit, Tag, And Push On Approval

On approval the command SHALL write the resolved version into the marketing
site's configuration, refresh the changelog page's authored date, commit both
together with the notes file, create the target tag on that commit by invoking
the existing version-bump script, and push `master` together with the tag.

The site constant SHALL be written by the command rather than by the version-bump
script, which creates the tag on an existing commit and therefore runs after the
commit the constant must belong to.

The changelog page carries an authored date that the site's build requires and
never derives. Because that page's content changes with every release, a date
left untouched would misreport the page's freshness in `sitemap.xml` from the
next release onward. The date SHALL NOT be set ahead of the build's own clock:
the site build rejects a date in the future, compared in UTC, so a date taken
from a local calendar ahead of UTC would fail the build rather than publish.

The push SHALL originate from the developer's checkout. A commit pushed by CI
would carry the workflow token, which does not trigger further workflow runs, so
the site would never redeploy.

#### Scenario: Approval commits notes, tags, and pushes

- **WHEN** the user approves at the gate for target version `0.6.0`
- **THEN** the command commits `releases/v0.6.0.md`, creates tag `v0.6.0` on that commit, and pushes `master` and the tag to the remote

#### Scenario: The release commit carries the site version

- **WHEN** the user approves at the gate for target version `0.6.0`
- **THEN** the command SHALL write `0.6.0` into the site's configuration before committing
- **AND** that file SHALL be part of the same commit as the notes file
- **AND** the push SHALL therefore match the site workflow's path filter

#### Scenario: The release commit refreshes the changelog page's date

- **WHEN** the user approves at the gate
- **THEN** the command SHALL set the changelog page's authored date to the release date
- **AND** that file SHALL be part of the same commit as the notes file
- **AND** the date SHALL NOT be later than the current UTC date

### Requirement: Follow The Build To Publication Or Recovery

After pushing, the command SHALL follow the release workflow run and SHALL NOT consider the release complete until the workflow finishes. On success it SHALL report the published release. On failure it SHALL report the failing job and offer recovery, given that a failed build leaves the tag pushed but no release published.

#### Scenario: Successful build reports the published release

- **WHEN** the release workflow completes successfully for the pushed tag
- **THEN** the command reports the release as published with a link to it

#### Scenario: Failed build offers recovery

- **WHEN** the release workflow fails for the pushed tag
- **THEN** the command reports the failing job
- **AND** offers recovery options (such as deleting the tag and re-releasing the same version, or shipping a follow-up patch)

### Requirement: Final Releases Only

In its first version the command SHALL operate on final `x.y.z` versions only and SHALL decline prerelease inputs (such as `-rc` or `-beta` suffixes).

#### Scenario: Prerelease input is declined

- **WHEN** the user supplies an explicit version containing a prerelease suffix (for example `1.0.0-rc.1`)
- **THEN** the command declines it and explains that prerelease versions are not supported

