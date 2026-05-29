## Context

The release pipeline (`.github/workflows/release.yml`) cross-compiles Windows on an `ubuntu-latest` runner via `cargo-xwin`, targeting `x86_64-pc-windows-msvc`, and currently uploads only the NSIS setup `.exe` from `target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe`. `cargo tauri build` always runs `cargo build --release` before bundling, so the raw application binary `target/x86_64-pc-windows-msvc/release/specforge.exe` already exists on disk after every Windows build — it is simply never uploaded.

SpecForge has no `externalBin`, no bundled `resources`, and no sidecars; the frontend is compiled into the binary via the `custom-protocol` feature. The binary is therefore self-contained except for the one runtime every Tauri-on-Windows app needs: the Microsoft Edge **WebView2** runtime, which renders the app's window. The NSIS installer guarantees WebView2 via Tauri's `webviewInstallMode` (a bootstrapper run at install time); a raw binary does not — it uses whatever WebView2 is already on the machine.

## Goals / Non-Goals

**Goals:**
- Publish a single-file, no-install Windows executable as a release asset, alongside the existing NSIS installer.
- Reuse the existing `cargo-xwin` cross-compile output — no second build, no `tauri.conf.json` change, no new build flags.
- Name and document the asset so users understand what it is and its one prerequisite.

**Non-Goals:**
- Bundling or bootstrapping WebView2 with the portable binary (the `fixedRuntime` / fat-portable path).
- Trace-free portability: the running app still writes `%APPDATA%` (settings, workspace registry) and an autostart registry key. "Portable" here means "no installer," not "no footprint."
- Code signing — the portable exe stays unsigned, consistent with the rest of the pipeline.
- Producing an `.msi` (already out of scope per the existing spec; WiX is Windows-host-only).

## Decisions

**1. Ship the *thin* portable (raw binary, system WebView2) — not the fat portable (`fixedRuntime`).**
The thin path is a rename + upload of a file that already exists: near-zero added build time and bytes. Its only cost is the WebView2 system dependency, which in 2026 is effectively always satisfied — the Evergreen WebView2 Runtime ships with Windows 11 and has been present on maintained Windows 10 for years (Win10 reached EOL Oct 2025).
*Alternative — `webviewInstallMode: fixedRuntime`:* produces a truly offline-capable build, but it is a ~150 MB *folder* rather than one file, requires vendoring a fixed WebView2 runtime, and complicates the cargo-xwin cross-compile. It earns its weight only for air-gapped/legacy targets, which are not a goal. Rejected.

**2. Source the artifact from `target/x86_64-pc-windows-msvc/release/specforge.exe`, renamed at upload time.**
The crate package name is `specforge`, so the cargo output is lowercase, unversioned `specforge.exe` at the workspace-root target dir (not under `bundle/`). The workflow copies it to `SpecForge_${VERSION}_x64-portable.exe` (`VERSION` derived from the tag as `${GITHUB_REF_NAME#v}`, matching the stamping step). The `portable` marker and version keep it unambiguous next to `SpecForge_${VERSION}_x64-setup.exe`.
*Alternative — a Tauri bundle target:* there is no Tauri bundle target that emits a bare portable exe; the binary is a cargo artifact, not a bundle. Renaming the cargo output is the correct seam.

**3. Inject the WebView2 prerequisite into release notes without losing auto-generated notes.**
The release step uses `generate_release_notes: true`. `softprops/action-gh-release@v2` prepends a `body`/`body_path` to the auto-generated notes rather than replacing them, so a short fixed preamble naming the WebView2 requirement (and pointing WebView2-less machines at the installer) can coexist with the generated changelog.
*Alternative — a README/docs note only:* release-asset choices are made on the releases page, so the note belongs there. A docs note can supplement but not substitute.

**4. The release-collection job needs no change.**
`actions/download-artifact` with `merge-multiple: true` then `files: dist/**/*` already globs recursively, so adding the portable exe to the Windows job's `upload-artifact` path is sufficient for it to reach the release.

## Risks / Trade-offs

- **Portable exe launches to a blank/failed window on a machine without WebView2** → Mitigated by the documented prerequisite in release notes and by keeping the installer (which bootstraps WebView2) as the default recommendation. Realistically near-zero affected machines in 2026.
- **Users mistake the portable exe for the installer (or vice versa)** → Mitigated by explicit `-portable` vs `-setup` filename markers, both version-stamped.
- **SmartScreen "unrecognized app" prompt on the unsigned portable exe** → Same as today's unsigned installer; no regression. Signing is tracked as future work for all Windows artifacts together.
- **Future binary rename** (`[[bin]]` name or package rename) silently breaks the copy step → The copy targets a hardcoded `specforge.exe`; if it ever disappears the step fails loudly (`if-no-files-found: error` on the upload). Acceptable.

## Open Questions

- None blocking. The exact preamble wording for the WebView2 note is a copy decision left to implementation; the spec only requires that the prerequisite be stated.
