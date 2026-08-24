# release-pipeline Specification

## Purpose

Defines the tag-driven release workflow (`.github/workflows/release.yml`): stamping the version from a pushed `v*` tag into `Cargo.toml` and `crates/specforge/tauri.conf.json`, then building — unsigned, serialized, on three jobs — the Linux `.deb`/`.AppImage`, the Windows NSIS setup and portable `.exe` cross-compiled with `cargo-xwin`, the macOS universal `.app`/`.dmg`, and the standalone `specforge-tui` and `specforge-serve` archives, and publishing them together as one GitHub Release whose body is the committed `releases/<tag>.md`. It also owns the caveats that published body must carry: the portable build's WebView2 prerequisite, the Gatekeeper quarantine workaround for the unsigned macOS CLI binaries, and the serve binary's unauthenticated non-loopback bind. It starts at the pushed tag and stops at the published release — it does not author the notes or push the tag (`release-command`), and it is not the per-push build-and-test gate (`continuous-integration`).
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

### Requirement: Standalone Serve Binary Emitted For Each Platform

The pipeline SHALL produce a standalone `specforge-serve` command-line executable for macOS (universal), Linux (x64 and arm64), and Windows (x64) as release assets, in addition to the GUI bundles and the `specforge-tui` binaries. Each serve binary SHALL be built with the plain Rust toolchain — `cargo` natively, a musl cross-compile for the Linux targets, `cargo-xwin` for the Windows cross-compile — NOT through Tauri bundling, and SHALL be built within the existing per-platform build job, reusing that job's runner, toolchain, and cache.

#### Scenario: Serve binary appears on the release for every platform

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains a standalone `specforge-serve` asset for macOS, one for Linux `x64`, one for Linux `arm64`, and one for Windows, alongside the GUI bundles and the `specforge-tui` assets

#### Scenario: Serve binaries are built without Tauri bundling

- **WHEN** a build job produces its `specforge-serve` binary
- **THEN** the binary is produced by a direct `cargo`/`cargo-xwin` build of the `specforge-web` package's `specforge-serve` binary target, not by `tauri build` and not as a Tauri `externalBin` sidecar inside the GUI bundle

#### Scenario: No new runner is introduced for the serve binary

- **WHEN** the pipeline builds the serve binaries
- **THEN** each is built inside the platform's existing build job (the Linux, Windows, and macOS jobs that already produce the GUI bundles and TUI binaries), with no additional job or runner added
- **AND** both Linux targets are cross-compiled inside the existing Linux job rather than on a separate runner

#### Scenario: macOS serve binary is universal

- **WHEN** the macOS build job produces its `specforge-serve` binary
- **THEN** the binary, inspected with `lipo -info`, contains both `arm64` and `x86_64` architectures

#### Scenario: Windows serve binary is cross-compiled on a Linux runner

- **WHEN** the pipeline produces the Windows `specforge-serve.exe`
- **THEN** it is cross-compiled on a runner labelled `ubuntu-latest` via `cargo-xwin` targeting `x86_64-pc-windows-msvc`, not on a `windows-latest` runner

### Requirement: Serve Binary Embeds The Built Frontend Bundle

Each released `specforge-serve` binary SHALL contain the built frontend bundle, so a downloaded binary serves the web UI with no adjacent files. Because the bundle is embedded at compile time from the repository's build output directory, each build job SHALL build the frontend before compiling the serve binary. A serve binary that compiled against an absent or empty bundle still links and runs — it degrades to a build hint instead of the UI — so the pipeline SHALL verify the embedded bundle is present rather than relying on compilation succeeding.

#### Scenario: Frontend is built before the serve binary is compiled

- **WHEN** a build job compiles `specforge-serve`
- **THEN** the frontend bundle has already been produced in that job (the job's existing Tauri build runs the frontend build as part of its own work), so the embed reads a populated directory

#### Scenario: An empty bundle fails the build rather than shipping

- **WHEN** a build job would produce a `specforge-serve` binary whose embedded bundle contains no application entry document
- **THEN** the job fails and no release is published, rather than shipping a binary that answers requests with a build hint

### Requirement: Serve Binaries Packaged As Compressed Archives

Each standalone `specforge-serve` binary SHALL be distributed as a compressed archive — `.tar.gz` for macOS and Linux, `.zip` for Windows — rather than as a raw binary, so the executable permission bit is preserved on extraction. Each archive SHALL contain the single `specforge-serve` executable. The upload of each archive SHALL fail the build job if the archive is absent (i.e. retain `if-no-files-found: error`), so a missing serve artifact blocks release publication rather than shipping silently.

#### Scenario: Unix archives are tarballs, Windows is a zip

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the macOS and Linux `specforge-serve` assets are `.tar.gz` archives and the Windows `specforge-serve` asset is a `.zip` archive

#### Scenario: Extracted unix binary keeps its executable bit

- **WHEN** the macOS or Linux `.tar.gz` is extracted on a unix machine
- **THEN** the contained `specforge-serve` is executable directly (its mode bit is set) without a manual `chmod +x`

#### Scenario: Archive filename is versioned and identifies platform and arch

- **WHEN** the tag `v0.2.0` triggers the pipeline
- **THEN** each serve archive's filename includes `0.2.0`, the `specforge-serve` name, and a platform/arch marker (e.g. `specforge-serve_0.2.0_macos-universal.tar.gz`, `specforge-serve_0.2.0_linux-x64.tar.gz`, `specforge-serve_0.2.0_linux-arm64.tar.gz`, `specforge-serve_0.2.0_windows-x64.zip`), so it is distinguishable from the GUI bundles, from the TUI archives, and from the other platforms and architectures

#### Scenario: Missing serve archive blocks the release

- **WHEN** a build job's serve build or packaging produces no archive
- **THEN** that job's upload step fails (`if-no-files-found: error`) and, because the publish job depends on all build jobs, no GitHub Release is published for the tag

### Requirement: Serve Binaries Unsigned

The pipeline SHALL produce the standalone `specforge-serve` binaries without code signing on any platform, consistent with the pipeline's unsigned-artifacts requirement for the GUI bundles and TUI binaries.

#### Scenario: No signing applied to the serve binaries

- **WHEN** the pipeline produces a `specforge-serve` binary or archive
- **THEN** no `codesign`, `xcrun notarytool`, `signtool`, or other signing tooling is invoked against it

### Requirement: Serve Binary Version Matches Tag

The version of each standalone `specforge-serve` binary SHALL match the tag that triggered the build, with the leading `v` stripped. The binary inherits `[workspace.package].version`, which the pipeline already stamps from the tag before building, and the archive filename SHALL encode that version.

#### Scenario: Serve archive filename version matches the tag

- **WHEN** the tag `v0.2.0` triggers the pipeline
- **THEN** each `specforge-serve` archive's filename includes `0.2.0`, matching the stamped `[workspace.package].version` the binary was built from

### Requirement: Serve macOS Gatekeeper Quarantine Documented

Because the macOS `specforge-serve` binary is unsigned and, being a terminal executable, has no Gatekeeper "right-click ▸ Open" dialog, the release SHALL document how to clear the quarantine flag so a browser-downloaded binary will run.

#### Scenario: Release notes state the macOS quarantine workaround for the serve binary

- **WHEN** a release is published that includes the macOS `specforge-serve` archive
- **THEN** the release notes state that the macOS serve binary is unsigned and document clearing the quarantine flag (e.g. `xattr -dr com.apple.quarantine specforge-serve`) so it can be run

### Requirement: Network Bind Exposure Documented For Downloaders

Because the released `specforge-serve` binary can publish an unauthenticated read API on a network interface, the release SHALL document that the binary binds loopback by default and that requesting a non-loopback bind serves the workspace-reading API to everyone who can reach the port, without authentication.

#### Scenario: Release notes state the network-bind posture

- **WHEN** a release is published that includes the `specforge-serve` archives
- **THEN** the release notes state that the server binds loopback by default, and that a non-loopback bind is unauthenticated and should be used only on a trusted network

### Requirement: Linux Serve Binaries Statically Linked Against musl

The Linux `specforge-serve` binaries SHALL be built for musl targets and statically linked, so that a released binary runs on any Linux distribution regardless of its C library or C-library version. The pipeline SHALL NOT ship a Linux `specforge-serve` binary dynamically linked against the build runner's glibc, because the runner's glibc is newer than that of common long-term-support distributions and such a binary fails to start on them.

The workspace links no C library that prevents this: git, Tailscale, and WSL integration are invoked as subprocesses, and the HTTPS client uses a pure-Rust TLS implementation. No source or dependency change is required to satisfy this requirement.

#### Scenario: Released Linux serve binary is static

- **WHEN** a released Linux `specforge-serve` binary is inspected for dynamic dependencies
- **THEN** it reports no dynamic interpreter and no shared library dependencies

#### Scenario: Binary runs on an older glibc distribution

- **WHEN** a released Linux `specforge-serve` binary is run on a distribution whose glibc is older than the build runner's
- **THEN** it starts and serves normally, rather than failing with a C-library version error

#### Scenario: Binary runs on a musl distribution

- **WHEN** a released Linux `specforge-serve` binary is run on a musl-based distribution such as Alpine
- **THEN** it starts and serves normally

#### Scenario: GUI bundles keep dynamic linking

- **WHEN** the pipeline builds the Linux `.deb` and `.AppImage` bundles
- **THEN** those bundles remain dynamically linked against the system webkit and GTK libraries, unaffected by this requirement

### Requirement: Linux arm64 Serve Binary Emitted

The pipeline SHALL produce a standalone `specforge-serve` executable for Linux `arm64` in addition to Linux `x64`, cross-compiled inside the existing Linux build job. The pipeline SHALL NOT introduce an additional job or runner to produce it.

#### Scenario: Linux arm64 serve asset appears on the release

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains a Linux `arm64` `specforge-serve` archive alongside the Linux `x64` one

#### Scenario: arm64 cross-compile uses no extra runner

- **WHEN** the pipeline builds the Linux `arm64` serve binary
- **THEN** it is cross-compiled within the existing Linux build job, with no additional job or runner added

#### Scenario: Missing arm64 archive blocks the release

- **WHEN** the Linux job's `arm64` serve build or packaging produces no archive
- **THEN** that job's upload step fails and no GitHub Release is published for the tag

### Requirement: macOS Per-Architecture Serve Slices Retained

The macOS build job already compiles the `x86_64-apple-darwin` and `aarch64-apple-darwin` `specforge-serve` binaries separately before merging them into the universal binary. The job SHALL retain both single-architecture binaries as job artifacts, so that downstream publication can consume them without recompiling. Retaining them SHALL NOT replace or alter the universal archive published as a release asset.

#### Scenario: Both darwin slices are available downstream

- **WHEN** the macOS build job completes
- **THEN** the single-architecture `x86_64` and `arm64` `specforge-serve` binaries are both available to downstream jobs as artifacts

#### Scenario: Universal release asset is unchanged

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the macOS `specforge-serve` release asset remains the universal archive, containing a binary with both architecture slices

#### Scenario: No additional macOS compilation is introduced

- **WHEN** the macOS job produces the retained slices
- **THEN** they are the binaries the job already built for the universal merge, with no additional compilation step

### Requirement: Prerelease Tags Published As Prereleases

A tag whose version carries a prerelease suffix SHALL be published as a GitHub prerelease and SHALL NOT be designated the latest release. The pipeline SHALL derive both properties from the tag rather than hard-coding them, using the same prerelease test the npm channel applies to choose its dist-tag, so that a tag is classified identically by both channels. Requesting both prerelease and latest is rejected by the publishing API, and omitting the latest designation entirely defaults it to true, so it SHALL be set explicitly in both cases.

Because release assets must be attached before a release becomes visible, a prerelease SHALL be created as a draft, have its assets uploaded, and then be published — the ordering a stable release already follows. The npm publication job SHALL NOT run against a release still in draft state.

#### Scenario: A prerelease tag is marked as a prerelease

- **WHEN** a tag whose version carries a prerelease suffix triggers the pipeline
- **THEN** the resulting GitHub Release is marked as a prerelease
- **AND** it is not designated the latest release, so links to the latest release continue to resolve to the most recent stable one

#### Scenario: A stable tag is unaffected

- **WHEN** a tag whose version carries no prerelease suffix triggers the pipeline
- **THEN** the resulting GitHub Release is not marked as a prerelease and is designated the latest release

#### Scenario: Both channels classify a tag the same way

- **WHEN** any tag triggers the pipeline
- **THEN** a tag published as a GitHub prerelease is the same tag npm publishes under its non-default dist-tag, and a tag published as a final release is the one npm publishes under its default dist-tag

#### Scenario: A prerelease is fully assembled before it becomes visible

- **WHEN** the pipeline publishes a prerelease
- **THEN** the release is created as a draft, its assets are uploaded, and it is published afterwards
- **AND** the npm publication job runs only once that release is no longer a draft

### Requirement: Only Shipped Bundle Targets Are Built

The pipeline SHALL build only the bundle formats it publishes as release assets. A format that is built and then discarded costs build time on every release and can fail the build for reasons that never affect a shipped artifact — a version string a shipped format accepts but a discarded one rejects would fail a release for a bundle nobody receives.

#### Scenario: No unshipped bundle format is produced

- **WHEN** the pipeline builds the application bundles
- **THEN** it produces only the formats attached to the release, and does not build a format that the upload step would discard

#### Scenario: A discarded format cannot fail a release

- **WHEN** a version string is valid for every published bundle format but invalid for a format that is not published
- **THEN** the release completes, because that format is never built

### Requirement: npm Publication Job Gated On Release Publication

The pipeline SHALL include a publication job that publishes the release's `specforge-serve` binaries to the npm registry, and that job SHALL depend on the release-publication job succeeding. The publication job SHALL consume artifacts produced by the build jobs and SHALL NOT compile. Its publication semantics — package graph, ordering, dist-tag selection, and provenance — are defined by the `npm-distribution` capability.

#### Scenario: Publication runs only after the release is published

- **WHEN** the release-publication job succeeds for a tag
- **THEN** the npm publication job runs for that tag

#### Scenario: A failed release prevents publication

- **WHEN** the release-publication job fails for a tag
- **THEN** the npm publication job does not run and nothing is published to the registry

#### Scenario: Publication job performs no build

- **WHEN** the npm publication job runs
- **THEN** it downloads build-job artifacts and invokes no Rust or frontend build

