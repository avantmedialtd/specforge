## 1. Linux TUI binary (build-linux job)

- [x] 1.1 After the `Tauri build` step, add a `Build TUI binary` step: `cargo build --release -p specforge-tui` (no extra system deps — the TUI is pure terminal)
- [x] 1.2 Add a `Package TUI archive` step: `tar -czf specforge-tui_${VERSION}_linux-x64.tar.gz -C target/release specforge-tui` (preserve the executable bit; `VERSION="${GITHUB_REF_NAME#v}"`)
- [x] 1.3 Add the archive to the `Upload Linux bundles` `path:` (keep `if-no-files-found: error`)

## 2. Windows TUI binary (build-windows job)

- [x] 2.1 After the `Tauri build` step, add a `Build TUI binary` step: `cargo xwin build --release -p specforge-tui --target x86_64-pc-windows-msvc`
- [x] 2.2 Add a `Package TUI archive` step: `zip -j specforge-tui_${VERSION}_windows-x64.zip target/x86_64-pc-windows-msvc/release/specforge-tui.exe`
- [x] 2.3 Add the archive to the `Upload Windows bundles` `path:` (keep `if-no-files-found: error`)

## 3. macOS universal TUI binary (build-macos job)

- [x] 3.1 After the `Tauri build` step, add a `Build TUI binary` step building both arches: `cargo build --release -p specforge-tui --target x86_64-apple-darwin` and `--target aarch64-apple-darwin`
- [x] 3.2 `lipo -create -output specforge-tui` from the two arch binaries; verify with `lipo -info` that it contains both `arm64` and `x86_64`
- [x] 3.3 Add a `Package TUI archive` step: `tar -czf specforge-tui_${VERSION}_macos-universal.tar.gz specforge-tui`
- [x] 3.4 Add the archive to the `Upload macOS bundles` `path:` (keep `if-no-files-found: error`)

## 4. Release notes / documentation

- [x] 4.1 Update the `/release` skill's Downloads footer template with a TUI section listing the three archives per platform (`.claude/commands/release.md`)
- [x] 4.2 Document the macOS Gatekeeper quarantine workaround for the CLI: `xattr -dr com.apple.quarantine specforge-tui` (in the notes template, root README, and TUI README)
- [x] 4.3 Update `README.md` / `crates/specforge-tui/README.md` to note the TUI is downloadable from the GitHub Release (extract the archive, run `./specforge-tui`)

## 5. Verification

- [x] 5.1 Confirm the `release` publish job needs no change — `merge-multiple: true` + `files: dist/**/*` already collect and upload the new archives (verified: the new archives ride inside the existing `*-bundles` artifacts)
- [x] 5.2 Validate `release.yml` after edits — `actionlint` clean, YAML parses, all three new archive paths matched by their job's `upload-artifact`, and `if-no-files-found: error` still guards every upload (3/3)
- [x] 5.3 Verified locally on macOS (the novel recipe): both arches build → `lipo` universal (`arm64` + `x86_64`) → `tar` preserves the executable bit on extraction → the universal binary runs (`--status`, exit 0). NOTE: enumerating the Linux/Windows assets on a live release requires pushing a public `v*` tag (out of scope for apply); the `if-no-files-found: error` guard fails the build loudly if any archive is missing.
- [x] 5.4 Notes template verified to render the TUI Downloads section incl. the macOS quarantine line; the published-release render happens on the next real `v*` tag.
