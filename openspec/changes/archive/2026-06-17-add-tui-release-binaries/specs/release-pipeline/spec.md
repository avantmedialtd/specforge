## ADDED Requirements

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
