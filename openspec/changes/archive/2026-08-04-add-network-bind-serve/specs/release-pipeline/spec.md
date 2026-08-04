# release-pipeline

## ADDED Requirements

### Requirement: Standalone Serve Binary Emitted For Each Platform

The pipeline SHALL produce a standalone `specforge-serve` command-line executable for macOS (universal), Linux (x64), and Windows (x64) as release assets, in addition to the GUI bundles and the `specforge-tui` binaries. Each serve binary SHALL be built with the plain Rust toolchain — `cargo` natively, `cargo-xwin` for the Windows cross-compile — NOT through Tauri bundling, and SHALL be built within the existing per-platform build job, reusing that job's runner, toolchain, and cache.

#### Scenario: Serve binary appears on the release for every platform

- **WHEN** the pipeline completes successfully for a tag
- **THEN** the GitHub Release for that tag contains a standalone `specforge-serve` asset for macOS, one for Linux, and one for Windows, alongside the GUI bundles and the `specforge-tui` assets

#### Scenario: Serve binaries are built without Tauri bundling

- **WHEN** a build job produces its `specforge-serve` binary
- **THEN** the binary is produced by a direct `cargo`/`cargo-xwin` build of the `specforge-web` package's `specforge-serve` binary target, not by `tauri build` and not as a Tauri `externalBin` sidecar inside the GUI bundle

#### Scenario: No new runner is introduced for the serve binary

- **WHEN** the pipeline builds the serve binaries
- **THEN** each is built inside the platform's existing build job (the Linux, Windows, and macOS jobs that already produce the GUI bundles and TUI binaries), with no additional job or runner added

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
- **THEN** each serve archive's filename includes `0.2.0`, the `specforge-serve` name, and a platform/arch marker (e.g. `specforge-serve_0.2.0_macos-universal.tar.gz`, `specforge-serve_0.2.0_linux-x64.tar.gz`, `specforge-serve_0.2.0_windows-x64.zip`), so it is distinguishable from the GUI bundles, from the TUI archives, and from the other platforms

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
