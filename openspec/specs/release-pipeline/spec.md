# release-pipeline Specification

## Purpose
TBD - created by archiving change add-release-pipeline. Update Purpose after archive.
## Requirements
### Requirement: Tag-Driven Trigger

The release pipeline SHALL execute when, and only when, a git tag matching the glob `v*` is pushed to the repository.

#### Scenario: Pushing a version tag triggers the pipeline

- **WHEN** a tag matching `v*` (e.g. `v0.2.0`, `v1.0.0-rc.1`) is pushed to the repository
- **THEN** the release pipeline begins executing against the commit the tag points to

#### Scenario: Pushing a non-version tag does not trigger the pipeline

- **WHEN** a tag not matching `v*` (e.g. `nightly-2026-05-25`, `backup`) is pushed
- **THEN** the release pipeline does not execute

#### Scenario: Pushing a branch does not trigger the pipeline

- **WHEN** a commit is pushed to any branch without a tag
- **THEN** the release pipeline does not execute

### Requirement: Version Derived From Tag

The release pipeline SHALL derive the release version by stripping the leading `v` from the tag name, and SHALL stamp that version into `crates/specforge/tauri.conf.json` (`.version`) and the root `Cargo.toml` (`workspace.package.version`) before any build runs.

#### Scenario: Tag v0.2.0 stamps version 0.2.0

- **WHEN** the tag `v0.2.0` triggers the pipeline
- **THEN** `crates/specforge/tauri.conf.json` is rewritten with `"version": "0.2.0"` and `Cargo.toml`'s `workspace.package.version` is rewritten to `"0.2.0"` before any `cargo` or `tauri` command runs

#### Scenario: Pre-release tag preserves the pre-release suffix

- **WHEN** the tag `v0.2.0-rc.1` triggers the pipeline
- **THEN** the stamped version in both files is `0.2.0-rc.1`

#### Scenario: package.json is not stamped

- **WHEN** the pipeline runs for any tag
- **THEN** `package.json` is not modified by the pipeline and its `version` field is irrelevant to released artifacts

### Requirement: Linux Bundles Built on Linux Runner

The pipeline SHALL produce a `.deb` and an `.AppImage` artifact for Linux, built on an `ubuntu-latest` GitHub-hosted runner using the native Tauri toolchain.

#### Scenario: Linux artifacts are emitted

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains at least one `.deb` file and at least one `.AppImage` file

### Requirement: Windows Bundles Built on Linux Runner via cargo-xwin

The pipeline SHALL produce an NSIS setup `.exe` artifact for Windows, cross-compiled on an `ubuntu-latest` GitHub-hosted runner using `cargo-xwin` for the Rust toolchain. The pipeline does NOT produce an `.msi` artifact, because Tauri's MSI bundler (WiX-based) only runs on a Windows host and is incompatible with the Linux-runner constraint below.

#### Scenario: Windows artifacts are emitted

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains at least one setup `.exe` file (NSIS)

#### Scenario: No Windows-hosted runner is used for Windows artifacts

- **WHEN** the pipeline executes the Windows build job
- **THEN** that job runs on a runner with label `ubuntu-latest`, not `windows-latest`

### Requirement: macOS Universal Bundle Built on macOS Runner

The pipeline SHALL produce an `.app` bundle and a `.dmg` for macOS, built on a `macos-latest` GitHub-hosted runner with target `universal-apple-darwin` so the resulting bundle contains both `arm64` and `x86_64` slices.

#### Scenario: macOS artifacts are emitted

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains at least one `.dmg` file

#### Scenario: macOS bundle is universal

- **WHEN** the macOS build job completes
- **THEN** the `.app` bundle's main executable, inspected with `lipo -info`, contains both `arm64` and `x86_64` architectures

### Requirement: Unsigned Artifacts

The pipeline SHALL produce artifacts without code signing on any platform. The pipeline SHALL NOT invoke `codesign`, `xcrun notarytool`, `signtool`, or any other signing tooling.

#### Scenario: No signing secrets are consumed

- **WHEN** the pipeline runs
- **THEN** no step references `secrets.APPLE_*`, `secrets.WINDOWS_CERTIFICATE_*`, or equivalent signing credentials

### Requirement: Release Job Depends on All Build Jobs

The pipeline SHALL include a release-publication job that depends on all platform build jobs completing successfully, and SHALL NOT publish a release if any build job fails.

#### Scenario: Failed Linux build prevents release publication

- **WHEN** the Linux build job fails
- **THEN** no GitHub Release is created for that tag

#### Scenario: Failed Windows build prevents release publication

- **WHEN** the Windows build job fails
- **THEN** no GitHub Release is created for that tag

#### Scenario: Failed macOS build prevents release publication

- **WHEN** the macOS build job fails
- **THEN** no GitHub Release is created for that tag

#### Scenario: Successful runs publish all artifacts together

- **WHEN** all three build jobs complete successfully
- **THEN** a single GitHub Release is created for the tag with every emitted bundle attached

### Requirement: Auto-Published Release

The pipeline SHALL publish the GitHub Release in non-draft, non-prerelease state on successful completion. The release SHALL be visible to users immediately without manual intervention.

#### Scenario: Release is published, not drafted

- **WHEN** the pipeline completes successfully
- **THEN** the resulting GitHub Release is in published state (visible on the public releases page, not in the maintainer's drafts inbox)

### Requirement: Concurrency Control

The pipeline SHALL serialize concurrent release runs so that two tags pushed in quick succession produce two releases in order, rather than racing.

#### Scenario: Sequential tags release sequentially

- **WHEN** tag `v0.2.0` is pushed and, before its pipeline completes, tag `v0.2.1` is pushed
- **THEN** the `v0.2.1` pipeline queues behind the `v0.2.0` pipeline and both releases publish in tag-push order

### Requirement: Bundle Version Matches Tag

The version reported by each built bundle SHALL match the tag that triggered the build, with the leading `v` stripped.

#### Scenario: macOS Info.plist version matches the tag

- **WHEN** the tag `v0.2.0` triggers the pipeline and the resulting `.app` is inspected
- **THEN** the `CFBundleShortVersionString` in `Contents/Info.plist` equals `0.2.0`

#### Scenario: Linux .deb control file version matches the tag

- **WHEN** the tag `v0.2.0` triggers the pipeline and the resulting `.deb` control metadata is inspected
- **THEN** the `Version:` field equals `0.2.0`

#### Scenario: Windows installer version matches the tag

- **WHEN** the tag `v0.2.0` triggers the pipeline and the resulting NSIS setup `.exe` is inspected
- **THEN** the filename includes `0.2.0` (e.g. `SpecForge_0.2.0_x64-setup.exe`) and the PE version resource records `0.2.0`

### Requirement: Windows Portable Executable Emitted Alongside Installer

The pipeline SHALL produce a single-file portable Windows executable as a release asset, in addition to the NSIS setup `.exe`. The portable executable is the raw cross-compiled application binary (`specforge.exe`), not an installer: it requires no installation step and is a single file, not a folder. It SHALL be built from the same `cargo-xwin` cross-compile that produces the NSIS bundle — no separate build invocation or `tauri.conf.json` change is introduced to produce it.

#### Scenario: Portable executable appears on the release

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains a portable Windows `.exe` in addition to the NSIS setup `.exe`

#### Scenario: Portable executable is a single self-contained file

- **WHEN** the portable Windows `.exe` is downloaded and run on a machine with the Edge WebView2 runtime present
- **THEN** the application launches directly with no installation step, from a single file with no accompanying folder of resources

#### Scenario: Portable executable filename is versioned and distinguishable from the installer

- **WHEN** the tag `v0.2.0` triggers the pipeline
- **THEN** the portable asset's filename includes both `0.2.0` and a `portable` marker (e.g. `SpecForge_0.2.0_x64-portable.exe`), so it is not confused with `SpecForge_0.2.0_x64-setup.exe`

#### Scenario: Portable executable is unsigned

- **WHEN** the pipeline produces the portable Windows `.exe`
- **THEN** no Authenticode signing step is applied to it, consistent with the pipeline's unsigned-artifacts requirement

### Requirement: Portable Executable WebView2 Prerequisite Documented

Because the portable executable does not bundle or bootstrap the Microsoft Edge WebView2 runtime (unlike the NSIS installer, which ensures it), the release SHALL document that the portable build depends on a system-provided WebView2 runtime.

#### Scenario: Release notes state the WebView2 prerequisite

- **WHEN** a release is published that includes the portable Windows `.exe`
- **THEN** the release notes state that the portable build requires the Edge WebView2 runtime (preinstalled on Windows 11 and maintained Windows 10), and that the installer is the alternative for machines lacking it

### Requirement: Release Body Sourced From Versioned Notes File

The release-publication job SHALL render the GitHub Release body from a versioned notes file committed at the tagged ref, at path `releases/<tag>.md` (the tag including its leading `v`). To do so the job SHALL check out the repository at the tagged ref so the file is present. The job SHALL NOT inline a static release body and SHALL NOT rely on GitHub's auto-generated release notes for the body.

#### Scenario: Body comes from the committed notes file

- **WHEN** tag `v0.6.0` triggers the pipeline and `releases/v0.6.0.md` exists at that commit
- **THEN** the published GitHub Release's body is the rendered contents of `releases/v0.6.0.md`

#### Scenario: Publication job checks out the repository

- **WHEN** the release-publication job runs
- **THEN** it checks out the repository at the tagged ref before resolving the notes file
- **AND** the body path `releases/${tag}.md` resolves to the committed file

#### Scenario: Auto-generated notes are not used for the body

- **WHEN** the pipeline publishes a release
- **THEN** the release body is not GitHub's auto-generated commit/PR list and is not a hard-coded inline body

### Requirement: Standalone TUI Binary Emitted For Each Platform

The pipeline SHALL produce a standalone `specforge-tui` command-line executable for macOS (universal), Linux (x64), and Windows (x64) as release assets, in addition to the GUI bundles. Each TUI binary SHALL be built with the plain Rust toolchain — `cargo` natively, `cargo-xwin` for the Windows cross-compile — NOT through Tauri bundling, and SHALL be built within the existing per-platform build job, reusing that job's runner, toolchain, and cache.

#### Scenario: TUI binary appears on the release for every platform

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains a standalone `specforge-tui` asset for macOS, one for Linux, and one for Windows, alongside the GUI bundles

#### Scenario: TUI binaries are built without Tauri bundling

- **WHEN** a build job produces its `specforge-tui` binary
- **THEN** the binary is produced by a direct `cargo`/`cargo-xwin` build of the `specforge-tui` package, not by `tauri build` and not as a Tauri `externalBin` sidecar inside the GUI bundle

#### Scenario: No new runner is introduced for the TUI

- **WHEN** the pipeline builds the TUI binaries
- **THEN** each is built inside the platform's existing build job (the Linux, Windows, and macOS jobs that already produce the GUI bundles), with no additional job or runner added

#### Scenario: macOS TUI binary is universal

- **WHEN** the macOS build job produces its `specforge-tui` binary
- **THEN** the binary, inspected with `lipo -info`, contains both `arm64` and `x86_64` architectures

#### Scenario: Windows TUI binary is cross-compiled on a Linux runner

- **WHEN** the pipeline produces the Windows `specforge-tui.exe`
- **THEN** it is cross-compiled on a runner labelled `ubuntu-latest` via `cargo-xwin` targeting `x86_64-pc-windows-msvc`, not on a `windows-latest` runner

### Requirement: TUI Binaries Packaged As Compressed Archives

Each standalone `specforge-tui` binary SHALL be distributed as a compressed archive — `.tar.gz` for macOS and Linux, `.zip` for Windows — rather than as a raw binary, so the executable permission bit is preserved on extraction. Each archive SHALL contain the single `specforge-tui` executable. The upload of each archive SHALL fail the build job if the archive is absent (i.e. retain `if-no-files-found: error`), so a missing TUI artifact blocks release publication rather than shipping silently.

#### Scenario: Unix archives are tarballs, Windows is a zip

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the macOS and Linux `specforge-tui` assets are `.tar.gz` archives and the Windows `specforge-tui` asset is a `.zip` archive

#### Scenario: Extracted unix binary keeps its executable bit

- **WHEN** the macOS or Linux `.tar.gz` is extracted on a unix machine
- **THEN** the contained `specforge-tui` is executable directly (its mode bit is set) without a manual `chmod +x`

#### Scenario: Archive filename is versioned and identifies platform and arch

- **WHEN** the tag `v0.2.0` triggers the pipeline
- **THEN** each TUI archive's filename includes `0.2.0`, the `specforge-tui` name, and a platform/arch marker (e.g. `specforge-tui_0.2.0_macos-universal.tar.gz`, `specforge-tui_0.2.0_linux-x64.tar.gz`, `specforge-tui_0.2.0_windows-x64.zip`), so it is distinguishable from the GUI bundles and from the other platforms

#### Scenario: Missing TUI archive blocks the release

- **WHEN** a build job's TUI build or packaging produces no archive
- **THEN** that job's upload step fails (`if-no-files-found: error`) and, because the publish job depends on all build jobs, no GitHub Release is published for the tag

### Requirement: TUI Binaries Unsigned

The pipeline SHALL produce the standalone `specforge-tui` binaries without code signing on any platform, consistent with the pipeline's unsigned-artifacts requirement for the GUI bundles.

#### Scenario: No signing applied to the TUI binaries

- **WHEN** the pipeline produces a `specforge-tui` binary or archive
- **THEN** no `codesign`, `xcrun notarytool`, `signtool`, or other signing tooling is invoked against it

### Requirement: TUI Binary Version Matches Tag

The version of each standalone `specforge-tui` binary SHALL match the tag that triggered the build, with the leading `v` stripped. The binary inherits `[workspace.package].version`, which the pipeline already stamps from the tag before building, and the archive filename SHALL encode that version.

#### Scenario: TUI archive filename version matches the tag

- **WHEN** the tag `v0.2.0` triggers the pipeline
- **THEN** each `specforge-tui` archive's filename includes `0.2.0`, matching the stamped `[workspace.package].version` the binary was built from

### Requirement: TUI macOS Gatekeeper Quarantine Documented

Because the macOS `specforge-tui` binary is unsigned and, being a terminal executable, has no Gatekeeper "right-click ▸ Open" dialog, the release SHALL document how to clear the quarantine flag so a browser-downloaded binary will run.

#### Scenario: Release notes state the macOS quarantine workaround for the CLI

- **WHEN** a release is published that includes the macOS `specforge-tui` archive
- **THEN** the release notes state that the macOS TUI binary is unsigned and document clearing the quarantine flag (e.g. `xattr -dr com.apple.quarantine specforge-tui`) so it can be run

