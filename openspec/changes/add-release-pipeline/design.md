# Design — Add Release Pipeline

## Context

SpecForge is a Tauri 2 desktop app that targets macOS 11+, Windows, and Linux (per the `tray-indicator` spec). `crates/specforge/tauri.conf.json` declares `bundle.targets: "all"`, which means each Tauri build emits the platform's full bundle set: `.deb` + `.AppImage` + `.tar.gz` on Linux, `.msi` + `.exe` on Windows, `.app` + `.dmg` on macOS.

There is no existing release tooling. Version `0.1.0` is duplicated across `crates/specforge/tauri.conf.json` (`"version": "0.1.0"`), root `Cargo.toml` (`workspace.package.version = "0.1.0"`), and `package.json` (`"version": "0.1.0"`). Only the first two are consumed by anything that ships — the Tauri bundler reads `tauri.conf.json`, and `cargo build` reads the workspace version. `package.json` is `private: true`, so its version field is ornamental for this app.

The Avant Media Ltd organisation does not yet have a Developer ID Application certificate. macOS signing and notarization are deferred to a separate change.

The companion `add-ci-pipeline` change establishes PR-time validation on a single Linux runner. This change layers tag-time multi-OS bundling on top; the two pipelines share no jobs and trigger on disjoint events.

## Goals / Non-Goals

**Goals:**

- Pushing a tag matching `v*` produces a published GitHub Release with bundles for all three OSes attached.
- The version baked into each bundle matches the tag exactly. No drift between "what the tag says" and "what `About SpecForge` shows in the app."
- Linux runners do as much work as possible — they handle Linux *and* Windows bundles via `cargo-xwin`. macOS uses a macOS runner because cross-compiling and bundling `.app`/`.dmg` on Linux is too compromised to be worth it.
- The pipeline is self-contained: a contributor who has never seen the repo can cut a release by tagging.
- Zero runner cost — public repo, all hosted runners unmetered.

**Non-Goals:**

- **Code signing.** macOS bundles ship unsigned (user sees Gatekeeper warning); Windows bundles ship unsigned (user sees SmartScreen warning). Signing is a separate change with separate prerequisites (paid Apple Developer Program, EV/OV code-signing certificate purchase). Out of scope here.
- **Notarization.** Depends on macOS signing. Out of scope.
- **Auto-updater.** The Tauri updater plugin is not in this app's dependency list and the release pipeline does not produce a `latest.json` manifest. Users update by downloading a new release. Adding the updater later means a new change that introduces the plugin, generates an updater keypair, and extends release.yml to publish `latest.json`.
- **Other distribution channels.** Homebrew Cask, winget, Linux package repos all read from GH Releases anyway; they can be added later without changing this pipeline.
- **Pre-release / RC handling.** All tags push to the main release stream. Tag `v0.2.0-rc.1` would publish a release with that exact title; whether to mark it pre-release is a future concern.

## Decisions

### Tag is the source of truth for version

Three files duplicate the version today: `tauri.conf.json`, root `Cargo.toml`, and `package.json`. Reconciling them at every release manually is exactly the kind of process step that gets forgotten right before a release.

The release workflow extracts the version from the tag (strip leading `v`), then stamps it into:

- `crates/specforge/tauri.conf.json` (`.version` field) — consumed by `tauri build` for the bundle metadata.
- Root `Cargo.toml` (`workspace.package.version`) — consumed by `cargo build` so `env!("CARGO_PKG_VERSION")` returns the right thing.

The stamping happens in-workflow against the tag's checked-out tree. The repo is *not* updated with the stamped version — there's no commit-back, no PR. The next development cycle starts from whatever was in the files before, and the next tag re-stamps. This avoids the bot-commit / tag-vs-branch confusion that plagues "version bump" workflows.

`package.json` is left at `0.0.0` permanently. Reasons:

1. It is `private: true`. Nothing publishes it.
2. The frontend doesn't read its own version at runtime.
3. Stamping a third file is more failure surface for zero observable benefit.

**Alternative considered:** Use `tauri.conf.json` as the source of truth and validate that the tag matches it (failing the release if not). Rejected: makes the tagging process two-step (bump file, commit, push, then tag) and the failure mode is "release didn't happen, dev has to figure out why."

**Alternative considered:** Use `cargo-release` or `release-please` for automated version bumping. Rejected as overkill for a single-binary workspace with no semver-significant API to track.

### Linux for Linux + Windows; macOS for macOS

| Bundle | Runner | Toolchain |
|---|---|---|
| `.deb`, `.AppImage` | `ubuntu-latest` | Native Rust + Tauri Linux deps |
| `.msi`, `.exe` | `ubuntu-latest` | `cargo-xwin` + Tauri Windows target |
| `.app`, `.dmg` | `macos-latest` | Native Rust (universal2) + Tauri macOS bundler |

Linux runners are the cheapest and fastest, so we use them wherever the cross-compile story is clean. `cargo-xwin` provides the Microsoft toolchain on Linux without Wine; combined with Tauri's bundler, this produces fully functional `.msi` / `.exe` on a Linux runner. Many production Tauri projects do this.

macOS is the holdout. Cross-compiling Rust to `aarch64-apple-darwin` requires the macOS SDK (Apple EULA grey area when distributed via `osxcross`), and `.dmg` creation needs `hdiutil`, which is macOS-only. Workarounds exist but produce subtly worse artifacts. With unmetered macOS runners on a public repo, the cost-of-runner is zero — so we just use the right tool.

**Alternative considered:** Pure-Linux for all three, with `.tar.gz` of an unsigned `.app` instead of `.dmg`. Rejected per the cost calculus above: the public-repo macOS minutes are free.

**Alternative considered:** Self-hosted macOS runner. Rejected — adds infrastructure burden for no benefit when hosted runners are free.

### Universal macOS binary

Build for `universal-apple-darwin` rather than separate `aarch64-apple-darwin` and `x86_64-apple-darwin` releases. Tauri 2's `--target universal-apple-darwin` produces a single `.app` containing both architectures, which is roughly twice the size but eliminates the "which one do I download" question for users on a tray app where ergonomics matter and download size is not a hot button. macOS minimum is already `11.0` per `tauri.conf.json` (covers both Intel-shipped Macs and Apple Silicon).

**Alternative considered:** Two separate macOS artifacts. Rejected as user-hostile for a small download.

### `cargo-xwin` for Windows builds on Linux

`cargo-xwin` ([github.com/rust-cross/cargo-xwin](https://github.com/rust-cross/cargo-xwin)) downloads the Microsoft CRT and Windows SDK headers on first run and configures Rust + clang for the `x86_64-pc-windows-msvc` target. Tauri 2 supports cross-compilation to Windows via `cargo-xwin`; the bundler produces real `.msi` (via WiX 3) and `.exe` (via NSIS) on Linux.

The catches:

- WiX runs via Wine on Linux to build the `.msi`. Slow but reliable.
- NSIS for Linux is in the Ubuntu apt repos; works without Wine.
- First run downloads ~1 GB of SDK; subsequent runs cache in `~/.cache/xwin`.

We install Wine + WiX + NSIS via apt, install `cargo-xwin` via `cargo install`, configure `tauri build --target x86_64-pc-windows-msvc --runner cargo-xwin`, and ship.

**Alternative considered:** Use a `windows-latest` runner with native MSVC. Rejected: ubuntu-latest is faster and we already have to install Tauri Linux deps for the Linux build anyway, so sharing the runner type cuts setup overhead.

### Single release job downstream of the matrix

The three build jobs each upload their bundles as workflow artifacts. A fourth `release` job runs `needs: [build-linux, build-windows-on-linux, build-macos]`, downloads all artifacts, creates the GitHub Release at the tag with `softprops/action-gh-release@v2`, attaches everything, and publishes.

If *any* matrix job fails, the release job doesn't run — no half-published release with two of three OSes. The release UI shows the failing build job; re-running the failed job + then re-running the release job completes the release.

**Alternative considered:** Each build job creates/updates the release as it finishes. Rejected: race conditions on the release-creation step (`gh release create` is not idempotent in a useful way), and partial releases are worse than no release.

### Auto-publish, not draft

Tag → bundles built → release published immediately, no human in the loop.

The argument for draft mode (catch broken artifacts before users see them) is real but weak for our case: we have CI gating on PRs, so a broken `master` is rare; tags are pushed deliberately by the maintainer; and a bad release can be deleted from the GH UI and re-published from a new tag (`v0.2.1`) in minutes. The friction of "tag, wait, log into GH, click publish" recurs every release; the cost of a bad release recurs rarely. Auto-publish wins on expected value.

Easy to flip later by changing one argument to `softprops/action-gh-release`.

### Concurrency: one release at a time

Set `concurrency: { group: release, cancel-in-progress: false }`. If two tags get pushed in quick succession (unusual but possible), the second one queues behind the first instead of racing. `cancel-in-progress: false` (not true) because we want both releases to publish, not just the latest one.

## Risks / Trade-offs

- **`cargo-xwin` cold-cache cost** → First-ever release run downloads ~1 GB of Windows SDK. Subsequent runs cache. Mitigation: GH Actions cache keyed on `cargo-xwin` version restores the SDK across runs. ~5 minutes added to first run.
- **Unsigned bundles are noisy on first-run** → macOS users see "SpecForge cannot be opened because the developer cannot be verified"; Windows users see "Windows protected your PC." Both have well-known workarounds (right-click → Open on Mac; "More info" → "Run anyway" on Windows) but degrade trust. Documented as known-limitation in the release notes template; the signing-and-notarization change closes this gap.
- **WiX on Linux via Wine is brittle** → Wine occasionally has Mono/CLR runtime issues that break WiX silently. If we hit this, fallback is to move the Windows job to a `windows-latest` runner — a one-line matrix change.
- **macOS universal builds take ~2× the time** → Compiling for both architectures roughly doubles the macOS job runtime. With unmetered macOS minutes and ~10-minute baseline build, this is ~20 minutes per release. Acceptable.
- **Version stamping is in-workflow only** → After release `v0.2.0` ships, the repo's `tauri.conf.json` still says `0.1.0`. This is by design but might surprise contributors who expect "the version in the file" to mean "the latest released version." Documented at the top of the release workflow's stamping step.
- **No semver enforcement** → A user could tag `v999.0.0` and the pipeline would happily publish it. We rely on maintainer discipline. Could add a validation step later if multi-maintainer becomes a thing.

## Migration Plan

1. Merge `add-ci-pipeline` first so PR validation is in place.
2. Land this change (`add-release-pipeline`) in a PR. The workflow file is added but does not run on PR open (it only triggers on tag push).
3. After merge to `master`, push `v0.1.0` as the first real release tag to exercise the pipeline end-to-end.
4. Verify all six bundle artifacts (`.deb`, `.AppImage`, `.msi`, `.exe`, `.app`, `.dmg`) appear on the GH Release.
5. Install each bundle on its target OS and confirm the app launches.
6. If anything is broken, delete the GH Release, delete the tag, fix the workflow, push `v0.1.1` (don't re-use `v0.1.0` — tag history matters).

Rollback: delete `.github/workflows/release.yml`. Existing GH Releases stay where they are; no automated cleanup.

## Open Questions

None. All decisions surfaced in the design conversation were resolved before drafting.
