## 1. npm Package Scaffolding

- [x] 1.1 Create `npm/wrapper/` holding the wrapper's published sources, and define its manifest — name `@avantmedia/specforge`, a `specforge-serve` bin entry, an explicit `files` allowlist, and no bundled executable — as `wrapperManifest()` in `npm/packaging.mjs` (`npm-distribution`: *Published Package Graph*). The manifest is generated rather than committed as a static file, which is what the *Every Published Package Carries The Tag Version* requirement demands
- [x] 1.2 Write the bin shim at `npm/wrapper/bin/specforge-serve.mjs`: resolve the installed platform package, execute its binary with every argument forwarded unchanged, inherit stdin/stdout/stderr, and exit with the child's status (`npm-distribution`: *Wrapper Exposes A specforge-serve Bin Shim*)
- [x] 1.3 Add the shim's unresolved-platform path: when no platform package resolves, exit non-zero with a message naming the detected `process.platform` and `process.arch` and pointing at the GitHub Releases downloads, never an unhandled module-resolution throw (`npm-distribution`: *Unresolvable Platform Fails With An Actionable Message*)
- [x] 1.4 Define the platform package manifest as `platformManifest()` in `npm/packaging.mjs`, driven by the `PLATFORMS` table — `os` and `cpu` set per target, exactly one executable in `files`, and **no `libc` field** on the Linux targets, asserted by a test so it cannot be "fixed" back in (`npm-distribution`: *Linux Platform Packages Omit The libc Field*, *Published Package Graph*). Generated for the same reason as 1.1, so there is no `npm/platform/` directory of static files
- [x] 1.5 Write `npm/build-packages.mjs`, which takes a version and an artifact directory and emits all six package directories with the version stamped and the wrapper's `optionalDependencies` pinned to that exact version; nothing it writes is committed (`npm-distribution`: *Every Published Package Carries The Tag Version*)
- [x] 1.6 Add a unit test for the shim's platform-resolution and failure branches, runnable via `bun test`, so the error path is covered without a published package

## 2. Linux Build Matrix — Static musl and arm64

- [x] 2.1 Install `cargo-zigbuild` and add the `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` targets to the `build-linux` job in `.github/workflows/release.yml`, keeping the work inside that job so no runner is added (`release-pipeline`: *Standalone Serve Binary Emitted For Each Platform*)
- [x] 2.2 Build `specforge-serve` for both musl targets after the existing frontend build, so the embedded `dist/` is populated at compile time (`release-pipeline`: *Linux Serve Binaries Statically Linked Against musl*)
- [x] 2.3 Add a job step asserting each musl binary is statically linked — no dynamic interpreter, no shared-library dependencies — so a regression to dynamic linking fails the build rather than shipping (`release-pipeline`: *Linux Serve Binaries Statically Linked Against musl*)
- [x] 2.4 Package and upload `specforge-serve_<version>_linux-x64.tar.gz` and `specforge-serve_<version>_linux-arm64.tar.gz`, retaining `if-no-files-found: error` (`release-pipeline`: *Linux arm64 Serve Binary Emitted*, *Serve Binaries Packaged As Compressed Archives*)
- [x] 2.5 Confirm the `.deb` and `.AppImage` bundles still build dynamically linked and are untouched by the musl work (`release-pipeline`: *Linux Serve Binaries Statically Linked Against musl*)

## 3. macOS Slice Retention

- [x] 3.1 In `build-macos`, upload the pre-`lipo` `x86_64-apple-darwin` and `aarch64-apple-darwin` `specforge-serve` binaries as job artifacts alongside the existing bundles (`release-pipeline`: *macOS Per-Architecture Serve Slices Retained*)
- [x] 3.2 Verify the universal `specforge-serve` archive published as a release asset is unchanged, and that no additional compilation was introduced (`release-pipeline`: *macOS Per-Architecture Serve Slices Retained*)

## 4. Publication Job

- [x] 4.1 Add a `publish-npm` job to `.github/workflows/release.yml` with `needs: [release]` and `permissions: id-token: write`, so a failed release publishes nothing (`release-pipeline`: *npm Publication Job Gated On Release Publication*)
- [x] 4.2 Have the job download the build jobs' artifacts and run `npm/build-packages.mjs` against them, invoking no Rust or frontend build (`npm-distribution`: *Published Binaries Are The Released Binaries*)
- [x] 4.3 Publish the five platform packages first and the wrapper last, aborting before the wrapper if any platform publish fails (`npm-distribution`: *Publication Ordered After The GitHub Release*)
- [x] 4.4 Derive the dist-tag from the version — `next` when the version carries a prerelease suffix, `latest` otherwise (`npm-distribution`: *Prerelease Tags Publish Under A Non-Default Dist-Tag*)
- [x] 4.5 Publish with provenance from the workflow's OIDC identity, with no long-lived registry token stored in secrets (`npm-distribution`: *Publication Attaches Build Provenance*)
- [x] 4.6 Make publication re-runnable for an already-pushed tag after a transient registry failure, without consuming a new version, by skipping any package already published at that exact version (`npm-distribution`: *Publication Ordered After The GitHub Release*). Implemented as idempotency in `npm/publish.mjs` rather than a `workflow_dispatch` input: re-running the failed job from the same run keeps the tag and the build artifacts, whereas a dispatch would rebuild everything and still need this skip to survive a partial publish

## 5. Registry and Identity Setup

- [ ] 5.1 Create the `@avantmedia` npm organization and confirm it owns the scope — this must complete before the first tag is pushed, or the first publish fails after the release is already public
- [ ] 5.2 Configure trusted publishing for all six package names so OIDC publication is authorized (`npm-distribution`: *Publication Attaches Build Provenance*)
- [ ] 5.3 Decide and record whether to defensively claim the currently-free unscoped `specforge-serve` and `specforge-tui` names; not required by this change, but unavailable later if taken

## 6. Documentation

- [x] 6.1 Update `README.md` so the headless-server section leads with `npx @avantmedia/specforge`, notes that a global install is the right choice for a persistently running server, and states that no quarantine step applies to an npm install (`npm-distribution`: *npm Installs Require No Quarantine Or Permission Workaround*)
- [x] 6.2 Update the README's Linux download guidance to reflect that the `specforge-serve` archive is now statically linked and runs on musl and older glibc distributions (`release-pipeline`: *Linux Serve Binaries Statically Linked Against musl*)
- [x] 6.3 Update the release-notes Downloads footer template to document the npm channel, name the published package, and state that the quarantine workaround applies to the archive and not to an npm install (`release-command`: *Notes Footer Documents Downloads And Caveats*)
- [x] 6.4 Confirm the root `package.json` remains `private: true` with name `specforge`, and that no committed manifest records a published package's release version (`product-identity`: *Application Crate and Package Names*)

## 7. Verification

- [x] 7.1 Run `cargo test` across the workspace — no Rust source changes are expected, so this is a regression check that the musl target work touched nothing
- [x] 7.2 Run `bun run build` (strict typecheck plus bundle) and `bun test` for the shim's unit tests
- [x] 7.3 Build both musl binaries locally and assert static linking, then run each and confirm the server starts and serves the embedded UI
- [x] 7.4 Run `npm/build-packages.mjs` against local artifacts, then `npm pack` all six and install the wrapper from the packed tarballs into a scratch directory; confirm exactly one platform package resolves and the `specforge-serve` bin runs (`npm-distribution`: *Platform Selection Without Install Scripts*)
- [x] 7.5 Repeat the local install with lifecycle scripts disabled to confirm the channel works under `--ignore-scripts` (`npm-distribution`: *Platform Selection Without Install Scripts*)
- [x] 7.6 Exercise the shim's failure path by removing the resolved platform package from the scratch install, and confirm an actionable message and non-zero exit rather than a module-resolution stack trace (`npm-distribution`: *Unresolvable Platform Fails With An Actionable Message*)
- [x] 7.7 Confirm argument forwarding and exit-code propagation through the shim, including a non-zero exit from a refused unsafe bind (`npm-distribution`: *Wrapper Exposes A specforge-serve Bin Shim*)
- [x] 7.8 Run `npm publish --dry-run` for all six packages and inspect the file lists — the wrapper carries no executable, each platform package carries exactly one, and the Linux manifests declare no `libc` field (`npm-distribution`: *Published Package Graph*, *Linux Platform Packages Omit The libc Field*)
- [x] 7.9 Manual smoke: start the locally installed `specforge-serve` and open the served UI in a browser, walking a registered workspace to confirm the embedded bundle is intact. The browser loop is the right surface here — the native shell is unchanged by this change — and the implementer runs it, never the user
