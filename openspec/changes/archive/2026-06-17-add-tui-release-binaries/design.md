## Context

The `specforge-tui` terminal frontend exists in the workspace and is exercised by CI (the Lint/Test jobs build it and discard the output), but the **release** pipeline never ships it. `release.yml` runs `bun tauri build` in each of three platform jobs; `tauri build` invokes `cargo build --release` scoped to the `specforge` GUI crate, which does **not** depend on `specforge-tui`, so the TUI is never compiled at release time. There is no `externalBin` entry in `tauri.conf.json`, so it is not bundled as a sidecar either, and every `upload-artifact` glob targets `target/**/bundle/...` or the staged GUI `specforge.exe`. Three independent gaps, each sufficient to keep the TUI out of a release.

This change makes the TUI a first-class release asset by **building and packaging it inside the existing per-platform build jobs** and adding it to their upload globs. The publish job is untouched: it already collects all build-job artifacts (`merge-multiple: true`) and uploads everything (`files: dist/**/*`).

Constraints that shape the design:
- The `release-pipeline` capability already documents the exact analogue — the **Windows portable `.exe`**, a raw cross-compiled binary shipped alongside the bundle from the same build, unsigned, versioned, documented. The TUI requirements mirror that precedent.
- Windows is cross-compiled on a Linux runner via `cargo-xwin`; macOS ships a universal (`lipo`-merged) artifact. The TUI must match both.
- No application code changes — this is a packaging/CI change only.

## Goals / Non-Goals

**Goals:**
- A downloadable `specforge-tui` for macOS (universal), Linux (x64), and Windows (x64) on every release.
- Reuse the three existing build jobs and their toolchains/caches — no new jobs, no new runners.
- Package as compressed archives so the executable bit survives download (no `chmod +x` step for the user).
- Document the macOS Gatekeeper quarantine workaround for a terminal binary.

**Non-Goals:**
- Linux ARM64 or Windows ARM64 TUI builds — x64 only for v1, matching the GUI's target coverage.
- Bundling the TUI inside the GUI app (sidecar/`externalBin`) — explicitly rejected; the TUI is a standalone download.
- `cargo install` / Homebrew / a curl|sh installer — out of scope; GitHub Release assets only for v1.
- Code signing or notarization — consistent with the pipeline's unsigned-artifacts stance.
- A `specforge-tui --version` flag — not wired today; filename versioning covers the requirement (see Open Questions).

## Decisions

**1. Build the TUI inside the existing per-platform jobs, after `tauri build`.**
Each job already has the right runner, toolchain, targets, and a warm Rust cache. Running the TUI build *after* `tauri build` reuses the already-compiled `openspec-app`/`openspec-core` dependency graph (the TUI shares it), so the added step is mostly just compiling `specforge-tui` itself. *Alternative — a dedicated `build-tui` job (matrix over OS):* rejected as redundant runners and a second cold cache for crates the release already compiles.

**2. Plain `cargo` / `cargo xwin`, not Tauri.**
The TUI is a normal binary with no frontend and no webview. Linux uses `cargo build --release -p specforge-tui`; Windows uses `cargo xwin build --release -p specforge-tui --target x86_64-pc-windows-msvc` (the job already installed `cargo-xwin`); macOS builds both Apple targets and `lipo -create`s them. *Alternative — register the TUI as a Tauri `externalBin` sidecar:* rejected — it buries the CLI inside the `.app`/`.deb`, target-triple-renamed and off `$PATH`, which defeats "downloadable standalone CLI."

**3. Compressed archives (`.tar.gz` unix, `.zip` Windows), not raw binaries.**
A browser download of a raw Mach-O/ELF lands without the executable bit, forcing `chmod +x`; an archive restores the stored mode on extraction and is the convention CLI users expect from `curl | tar`. The small extra `tar`/`zip` step is worth removing a user papercut. *Alternative — raw binaries (like the Windows portable `.exe`):* viable and simpler, but the GUI portable is a double-click `.exe` (no bit needed) whereas the unix TUI is run from a shell — the cases differ. (User-confirmed: archives.)

**4. The macOS Gatekeeper caveat is documented, not worked around.**
The unsigned GUI tells users "right-click ▸ Open"; a terminal binary has no such dialog, so the only path is clearing quarantine: `xattr -dr com.apple.quarantine specforge-tui`. This is captured as a documentation requirement here, mirroring the existing `release-pipeline` "Portable Executable WebView2 Prerequisite Documented" requirement (asset-caveat documentation lives in `release-pipeline`, not `release-command`).

**5. Version comes for free from the existing stamp step.**
`stamp-version` rewrites `[workspace.package].version`; `specforge-tui` is `version.workspace = true`, so the release-built binary inherits the tag version with no extra wiring. The archive **filename** encodes the version (e.g. `specforge-tui_0.2.0_linux-x64.tar.gz`), satisfying the version-match requirement without depending on a `--version` flag the binary does not yet expose.

## Risks / Trade-offs

- [Windows cross-compile of a non-Tauri binary] The job only cross-compiles via `tauri build --runner cargo-xwin` today; a bare `cargo xwin build -p specforge-tui` is new. → It is the canonical `cargo-xwin` invocation; `if-no-files-found: error` on the upload fails the job loudly if it yields nothing, so a regression can't silently ship an empty release.
- [Asset-name collision / confusion with GUI assets] A new family of `specforge-tui_*` archives joins the `SpecForge_*` bundles. → Distinct `specforge-tui` prefix plus platform/arch in every name keeps them unambiguous on the releases page.
- [Added release wall-clock] Three extra `cargo build`s. → Small relative to `tauri build`; shared crates are already compiled and the cache is warm; runs sequentially within each existing job (no new fan-out).
- [Unsigned CLI friction on macOS] Quarantine blocks a browser-downloaded binary. → Documented workaround in the notes; consistent with the project's existing unsigned posture for the GUI.

## Migration Plan

Additive CI/packaging change; no data, no runtime, no user-facing app behavior changes. The first tagged release after this lands simply gains `specforge-tui_*` archives among its assets. Rollback: remove the added build/upload steps from `release.yml` and the TUI lines from the notes template — the GUI bundles are entirely independent and unaffected.

## Open Questions

- **`specforge-tui --version`**: should the binary expose a `--version` flag (so the version is verifiable at runtime, not just in the filename)? Leaning: out of scope for v1 — the filename carries the version and that satisfies the spec; add the flag later if asked. If added, it would be a one-line app-code change outside this CI-only proposal's scope.
- **Linux libc baseline**: the `ubuntu-latest` build links against that runner's glibc; very old distros could see a `GLIBC_x` mismatch. Leaning: accept for v1 (same baseline as the `.deb`/`.AppImage`); revisit a musl/static target only if users report it.
- **Archive layout**: archive the bare executable at the archive root vs nested in a versioned folder. Leaning: bare executable at root — simplest `tar xzf && ./specforge-tui`.
