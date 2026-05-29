## ADDED Requirements

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
