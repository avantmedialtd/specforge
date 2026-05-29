## 1. Emit the portable executable in the Windows build job

- [x] 1.1 In `.github/workflows/release.yml`, in the `build-windows` job, after the `Tauri build (NSIS .exe)` step, add a step that copies `target/x86_64-pc-windows-msvc/release/specforge.exe` to `SpecForge_${VERSION}_x64-portable.exe`, deriving `VERSION` from the tag the same way the stamping step does (`${GITHUB_REF_NAME#v}`).
- [x] 1.2 Confirm the binary name: the crate package is `specforge`, so the raw cross-compile output is `specforge.exe` (lowercase, unversioned) at the workspace-root `target/x86_64-pc-windows-msvc/release/` — not under `bundle/`. (Verified via `cargo metadata`: the only `bin` target is named `specforge`.)
- [x] 1.3 Add the renamed portable `.exe` to the `Upload Windows bundles` `actions/upload-artifact@v4` path (keep the existing `bundle/nsis/*.exe` entry; add the portable file).
- [x] 1.4 Verify the release job needs no change: `softprops/action-gh-release@v2` already globs `dist/**/*` after `merge-multiple: true`, so the portable exe is collected automatically. (Left `files: dist/**/*` untouched.)

## 2. Document the WebView2 prerequisite

- [x] 2.1 Decide where the WebView2 note lives. `generate_release_notes: true` auto-generates notes, so add a `body`/`body_path` (appended, not replacing) to the release step, or a fixed preamble, stating the portable build's WebView2 requirement and pointing WebView2-less machines at the installer. (Added a `body:` preamble to the `Create GitHub Release` step; the auto-changelog appends after it.)
- [x] 2.2 Word the note to match the spec scenario: "the portable build requires the Edge WebView2 runtime (preinstalled on Windows 11 and maintained Windows 10); use the installer if your machine lacks it."

## 3. Verify against the spec

> Deferred: these need a real `v*` release build + a Windows machine, and the
> current release job hardcodes `make_latest: 'true'` / `prerelease: false`, so a
> throwaway tag would publish a public release marked "latest". Verify on the
> next genuine tagged release rather than cutting a throwaway one.

- [ ] 3.1 Push a throwaway pre-release tag (e.g. `v0.0.0-portable-test`) on a branch and confirm the release carries both `SpecForge_*_x64-setup.exe` and `SpecForge_*_x64-portable.exe`.
- [ ] 3.2 Download the portable exe on a Windows 11 machine (or VM) and confirm it launches with no install step and renders its window (proves system WebView2 is used).
- [ ] 3.3 Confirm the portable filename includes both the version and `portable`, and is unsigned (no signature in the PE).
- [ ] 3.4 Delete the throwaway tag/release.

## 4. Sync the spec

- [x] 4.1 After implementation lands, run the OpenSpec sync/archive flow so the new requirements merge into `openspec/specs/release-pipeline/spec.md`. (Done via `openspec archive`, which moves the change and syncs the delta into the main spec.)
