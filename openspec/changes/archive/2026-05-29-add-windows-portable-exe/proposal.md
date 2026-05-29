# Add Windows Portable Executable

## Why

Today the only Windows artifact on a release is the NSIS setup `.exe` — an installer that writes the app into Program Files, drops shortcuts, and ensures the WebView2 runtime is present. Some users don't want an installer: they want to download one file, double-click it, and run the app (from a Downloads folder, a USB stick, a locked-down corporate machine where they can't run installers, or just to try it without committing to an install).

The good news is the standalone binary already gets built and then discarded. `bun tauri build` runs `cargo build --release` first — producing `target/x86_64-pc-windows-msvc/release/specforge.exe` — and *then* wraps that binary in NSIS. SpecForge is genuinely single-file material: it has no `externalBin`, no bundled `resources`, and no sidecars; the frontend is embedded into the binary at compile time via the `custom-protocol` feature. So shipping a portable exe is almost entirely a matter of renaming and uploading a file the pipeline already produces.

## What Changes

- The `build-windows` job in `.github/workflows/release.yml` copies the raw compiled binary `target/x86_64-pc-windows-msvc/release/specforge.exe` to a versioned, clearly-labelled name (`SpecForge_<version>_x64-portable.exe`) and uploads it alongside the existing NSIS setup `.exe`.
- The GitHub Release for each tag gains a second Windows asset: the portable executable, sitting next to the installer.
- Release notes document the one prerequisite: the portable build relies on the system's Microsoft Edge **WebView2 runtime** (preinstalled on Windows 11 and all maintained Windows 10), whereas the installer guarantees it.
- No application code, no `tauri.conf.json`, and no build flags change. The portable artifact is a byproduct of the existing cross-compile, not a new build.

## Capabilities

### New Capabilities

<!-- None. This extends the existing release-pipeline capability. -->

### Modified Capabilities

- `release-pipeline`: In addition to the NSIS setup `.exe`, the Windows build emits a single-file portable executable as a release asset.

## Impact

- Modified file: `.github/workflows/release.yml` (rename/copy + upload step in `build-windows`; the release-job glob already recurses, so collection needs no change).
- Modified spec: `openspec/specs/release-pipeline/spec.md` gains a requirement for the Windows portable executable.
- **Deliberate scope boundaries** (so nobody "fixes" these later as oversights):
  - **"Portable" means "no installer," not "zero-footprint."** The running app still writes `settings.json` and the workspace registry to `%APPDATA%`, and `tauri-plugin-autostart` writes a registry Run key if launch-on-login is enabled. True trace-free portability (config co-located with the exe) is explicitly out of scope.
  - **WebView2 is not bundled.** This ships the *thin* portable, which uses the machine's existing WebView2 runtime. A fully self-contained build (`webviewInstallMode: fixedRuntime`, a ~150 MB folder rather than one file) is out of scope; it earns its weight only for offline/legacy targets, which are not a goal.
  - **Still unsigned.** Consistent with the existing release-pipeline decision, the portable exe is not Authenticode-signed, so Windows SmartScreen shows the same "unrecognized app" prompt as the installer does today. Signing remains future work for all Windows artifacts together.
