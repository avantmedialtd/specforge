# Add Release Pipeline

## Why

SpecForge has no way to distribute itself. Producing user-installable artifacts today requires a developer to run `bun tauri build` manually on each target OS, which is both inconsistent (different machines, different toolchains) and bottlenecked on owning hardware for every supported platform. A tag-driven GitHub Actions release pipeline removes the human and the hardware from the loop: pushing `v0.2.0` produces signed-eventually `.deb`, `.AppImage`, `.msi`, `.exe`, `.app`, and `.dmg` artifacts on the corresponding GitHub Release.

This is the second half of the build/release story; `add-ci-pipeline` already lands PR-time validation, so by the time a tag is pushed the source tree has already been linted, tested, and smoke-built.

## What Changes

- Add `.github/workflows/release.yml` triggered on push of any tag matching `v*`.
- **Pre-build stamping**: derive the version from the tag (`v0.2.0` → `0.2.0`) and rewrite `crates/specforge/tauri.conf.json` and the root `Cargo.toml`'s `workspace.package.version` in-place before any build runs. The tag is the source of truth; `package.json` is left at `0.0.0` permanently (it is `private: true` and never published).
- **Three-OS build matrix**:
  - `ubuntu-latest` → Linux bundles (`.deb`, `.AppImage`).
  - `ubuntu-latest` + `cargo-xwin` → Windows bundles (`.msi`, `.exe`).
  - `macos-latest` → macOS universal bundle (`.app`, `.dmg`), unsigned.
- **Auto-published GitHub Release**: a single release job depends on all three matrix jobs, creates a release at the tag, attaches every artifact, and publishes (not drafted) on success.
- Each matrix job uploads its bundles as workflow artifacts so the release job can collect them in one place.

## Capabilities

### New Capabilities

- `release-pipeline`: Tag-driven workflow that produces user-installable bundles for Linux, Windows, and macOS, stamps the version from the tag into the workspace metadata, and publishes a GitHub Release with all artifacts attached.

### Modified Capabilities

<!-- None. Distribution is a new concern. -->

## Impact

- New file: `.github/workflows/release.yml`. No application code changes.
- Repository now has a meaningful concept of "version" that lives in three files (`tauri.conf.json`, `Cargo.toml` workspace.package.version, `package.json`) and a fourth source of truth (the git tag). The pipeline reconciles three of those from the tag; `package.json` stays at `0.0.0` by deliberate decision. This is documented in the design doc so future contributors don't "fix" the apparent inconsistency.
- First tagged release surfaces whichever cross-compilation issues exist in `cargo-xwin` for our specific dependency set (`resvg`, `usvg`, Tauri plugins). Fixes happen on the release branch before retagging.
- **Explicit out-of-scope, future work**:
  - Code signing on macOS (Developer ID + notarization). Without it, users see Gatekeeper warnings on first run.
  - Code signing on Windows (Authenticode). Without it, users see SmartScreen warnings.
  - Auto-updater (`tauri-plugin-updater` + signed `latest.json`). Users update by re-downloading.
  - Homebrew Cask / winget / Linux package repos. Distribution is GH Releases only.
- Runner cost: zero — public repo, unlimited minutes on all hosted runners including macOS.
