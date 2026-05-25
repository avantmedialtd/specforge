## 1. Workflow scaffolding

- [ ] 1.1 Create `.github/workflows/release.yml` with `name: Release`.
- [ ] 1.2 Configure trigger: `on: push: tags: ['v*']`.
- [ ] 1.3 Add `concurrency: { group: release, cancel-in-progress: false }` so back-to-back tags serialize.
- [ ] 1.4 Add `permissions: { contents: write }` at the workflow level so the release job can create the GH Release.

## 2. Version stamping (shared step)

- [ ] 2.1 Author a reusable composite action or shell snippet that derives `VERSION` from `${GITHUB_REF_NAME#v}` and exports it.
- [ ] 2.2 Stamp `crates/specforge/tauri.conf.json`'s `.version` via `jq` or a small Python/sed script that's robust to formatting.
- [ ] 2.3 Stamp `Cargo.toml`'s `workspace.package.version` via `sed`/`toml-cli` — verify with `cargo metadata --format-version 1 | jq -r '.workspace_default_members[0]'` that the bump is visible to cargo.
- [ ] 2.4 Do NOT commit the stamped files back to the repo. They live in the workflow checkout only.

## 3. Linux build job

- [ ] 3.1 Add `build-linux` job on `ubuntu-latest`.
- [ ] 3.2 Check out via `actions/checkout@v4`.
- [ ] 3.3 Run the version-stamping step from §2.
- [ ] 3.4 Install Rust stable via `dtolnay/rust-toolchain@stable`.
- [ ] 3.5 Restore Rust cache via `Swatinem/rust-cache@v2` with `shared-key: "release-linux"`.
- [ ] 3.6 Install Tauri Linux system deps (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`) via `apt-get`.
- [ ] 3.7 Install bun via `oven-sh/setup-bun@v2`.
- [ ] 3.8 Run `bun install --frozen-lockfile`.
- [ ] 3.9 Run `bun tauri build` (default targets emit `.deb`, `.AppImage`).
- [ ] 3.10 Upload `crates/specforge/target/release/bundle/deb/*.deb` and `crates/specforge/target/release/bundle/appimage/*.AppImage` via `actions/upload-artifact@v4` with name `linux-bundles`.

## 4. Windows-on-Linux build job

- [ ] 4.1 Add `build-windows` job on `ubuntu-latest`.
- [ ] 4.2 Check out via `actions/checkout@v4`.
- [ ] 4.3 Run the version-stamping step from §2.
- [ ] 4.4 Install Rust stable via `dtolnay/rust-toolchain@stable` with `targets: x86_64-pc-windows-msvc`.
- [ ] 4.5 Restore Rust cache via `Swatinem/rust-cache@v2` with `shared-key: "release-windows"`.
- [ ] 4.6 Install `cargo-xwin` via `cargo install cargo-xwin --locked`.
- [ ] 4.7 Cache `~/.cache/xwin` (the Windows SDK download) keyed on `cargo-xwin` version.
- [ ] 4.8 Install Wine + NSIS via `apt-get install -y wine nsis` so the Tauri bundler can produce `.msi` (WiX via Wine) and `.exe` (NSIS).
- [ ] 4.9 Install bun via `oven-sh/setup-bun@v2`.
- [ ] 4.10 Run `bun install --frozen-lockfile`.
- [ ] 4.11 Run `bun tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc`.
- [ ] 4.12 Upload `crates/specforge/target/x86_64-pc-windows-msvc/release/bundle/msi/*.msi` and `crates/specforge/target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe` via `actions/upload-artifact@v4` with name `windows-bundles`.

## 5. macOS build job

- [ ] 5.1 Add `build-macos` job on `macos-latest`.
- [ ] 5.2 Check out via `actions/checkout@v4`.
- [ ] 5.3 Run the version-stamping step from §2.
- [ ] 5.4 Install Rust stable via `dtolnay/rust-toolchain@stable` with `targets: x86_64-apple-darwin, aarch64-apple-darwin`.
- [ ] 5.5 Restore Rust cache via `Swatinem/rust-cache@v2` with `shared-key: "release-macos"`.
- [ ] 5.6 Install bun via `oven-sh/setup-bun@v2`.
- [ ] 5.7 Run `bun install --frozen-lockfile`.
- [ ] 5.8 Run `bun tauri build --target universal-apple-darwin`.
- [ ] 5.9 Verify the resulting `.app` is universal: `lipo -info crates/specforge/target/universal-apple-darwin/release/bundle/macos/SpecForge.app/Contents/MacOS/SpecForge` reports both `x86_64` and `arm64`. Fail the job if not.
- [ ] 5.10 Upload `crates/specforge/target/universal-apple-darwin/release/bundle/dmg/*.dmg` via `actions/upload-artifact@v4` with name `macos-bundles`.

## 6. Release publication job

- [ ] 6.1 Add `release` job on `ubuntu-latest` with `needs: [build-linux, build-windows, build-macos]`.
- [ ] 6.2 Download all three artifact buckets via `actions/download-artifact@v4` with `pattern: '*-bundles'` and `merge-multiple: true` into a single directory.
- [ ] 6.3 Use `softprops/action-gh-release@v2` to create a release at `${{ github.ref_name }}`, attach every downloaded artifact, set `draft: false`, `prerelease: false`, and `generate_release_notes: true`.
- [ ] 6.4 Set `make_latest: true` so the release becomes the repository's "Latest" badge.

## 7. Verification

- [ ] 7.1 Land the PR (workflow file added; not triggered by PR open since trigger is tag-push).
- [ ] 7.2 After merge, push `v0.1.0` from `master`.
- [ ] 7.3 Confirm all three build jobs run on the correct runner labels per spec.
- [ ] 7.4 Confirm GH Release `v0.1.0` is created and published (not drafted) with `.deb`, `.AppImage`, `.msi`, `.exe`, and `.dmg` attached.
- [ ] 7.5 Download each bundle on its target OS; confirm app launches. Document Gatekeeper/SmartScreen workaround in release notes template for unsigned builds.
- [ ] 7.6 If anything is broken: delete the GH Release, delete the tag, fix the workflow, push `v0.1.1` (do not re-use `v0.1.0`).
