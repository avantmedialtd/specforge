## ADDED Requirements

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

## MODIFIED Requirements

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
