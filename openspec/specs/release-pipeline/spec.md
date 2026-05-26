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
