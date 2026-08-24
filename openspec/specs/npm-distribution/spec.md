# npm-distribution Specification

## Purpose
TBD - created by archiving change add-npm-distribution. Update Purpose after archive.
## Requirements
### Requirement: Published Package Graph

The project SHALL publish the standalone `specforge-serve` executable to the public npm registry as a wrapper package named `@avantmedia/specforge` together with one platform package per supported target, named `@avantmedia/specforge-<platform>` where `<platform>` is the npm `<os>-<cpu>` pair. The supported set SHALL be `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`, and `win32-x64`. The wrapper SHALL declare every platform package in `optionalDependencies`, pinned to the exact version being published, and SHALL itself contain no executable. The unscoped name `specforge` on the public registry belongs to an unrelated project and SHALL NOT be used — see the *Application Crate and Package Names* requirement in the `product-identity` capability.

#### Scenario: Wrapper declares every platform package

- **WHEN** the published `@avantmedia/specforge` package manifest is read
- **THEN** its `optionalDependencies` names all five platform packages
- **AND** each is pinned to the exact version of the wrapper, with no range operator

#### Scenario: Wrapper ships no binary

- **WHEN** the `@avantmedia/specforge` tarball is extracted
- **THEN** it contains the bin shim and package metadata but no `specforge-serve` executable

#### Scenario: Each platform package ships exactly one executable

- **WHEN** any `@avantmedia/specforge-<platform>` tarball is extracted
- **THEN** it contains a single `specforge-serve` executable (`specforge-serve.exe` on `win32`) and package metadata, and no other platform's binary

### Requirement: Platform Selection Without Install Scripts

Platform selection SHALL be performed by the package manager from each platform package's `os` and `cpu` manifest fields, so that installing the wrapper downloads exactly one platform package. The channel SHALL NOT use a `postinstall` script, SHALL NOT download binaries at install time, and SHALL NOT download binaries at first run.

#### Scenario: Only the matching platform package is downloaded

- **WHEN** a user installs `@avantmedia/specforge` on an `arm64` macOS machine
- **THEN** only `@avantmedia/specforge-darwin-arm64` is fetched
- **AND** the other four platform packages are skipped by the package manager

#### Scenario: Install succeeds with scripts disabled

- **WHEN** a user installs the wrapper with lifecycle scripts disabled (for example `npm install --ignore-scripts`)
- **THEN** the installation completes and the `specforge-serve` bin is runnable

#### Scenario: Install succeeds without registry-external network access

- **WHEN** the wrapper is installed from a private registry mirror or an offline cache, with no access to the releases host
- **THEN** the installation completes and the `specforge-serve` bin is runnable

### Requirement: Linux Platform Packages Omit The libc Field

Because the Linux binaries are statically linked against musl — see the *Linux Serve Binaries Statically Linked Against musl* requirement in the `release-pipeline` capability — they run on glibc-based distributions as well as musl-based ones. The Linux platform packages SHALL therefore omit the `libc` manifest field entirely, rather than declaring `musl`, so that package managers honouring `libc` do not exclude them on glibc systems.

#### Scenario: Linux platform manifests declare no libc constraint

- **WHEN** the `@avantmedia/specforge-linux-x64` or `@avantmedia/specforge-linux-arm64` manifest is read
- **THEN** it contains no `libc` field

#### Scenario: Linux package installs on a glibc distribution

- **WHEN** the wrapper is installed on a glibc-based `x64` Linux distribution
- **THEN** `@avantmedia/specforge-linux-x64` is selected and its binary runs

#### Scenario: Linux package installs on a musl distribution

- **WHEN** the wrapper is installed on a musl-based `x64` Linux distribution such as Alpine
- **THEN** `@avantmedia/specforge-linux-x64` is selected and its binary runs

### Requirement: Wrapper Exposes A specforge-serve Bin Shim

The wrapper package SHALL declare a bin named `specforge-serve` that resolves the installed platform package, executes its binary, forwards every command-line argument unchanged, inherits the parent process's standard input, output, and error streams, and exits with the child's exit code. The shim SHALL NOT interpret, rewrite, or validate the arguments it forwards.

#### Scenario: Arguments reach the binary unchanged

- **WHEN** a user runs the wrapper bin with `--bind` and `--port` arguments
- **THEN** the underlying `specforge-serve` binary receives those arguments exactly as given

#### Scenario: Exit code propagates

- **WHEN** the underlying binary exits with a non-zero status, for example after refusing an unsafe bind
- **THEN** the shim exits with that same status

#### Scenario: Server output reaches the terminal

- **WHEN** the server prints its startup address or a network-bind warning
- **THEN** that output appears on the invoking terminal, not buffered or discarded by the shim

### Requirement: Unresolvable Platform Fails With An Actionable Message

When the shim cannot resolve any platform package — because the host platform is unsupported, or because a package manager resolved the `optionalDependencies` to nothing — it SHALL exit non-zero with a message naming the detected operating system and CPU architecture and directing the user to the GitHub Releases downloads. It SHALL NOT surface an unhandled module-resolution error.

#### Scenario: Unsupported platform is reported by name

- **WHEN** the shim runs on a platform for which no package is published
- **THEN** it exits non-zero with a message stating the detected operating system and CPU architecture
- **AND** the message points to the GitHub Releases page as the alternative

#### Scenario: A missing optional dependency is reported, not thrown

- **WHEN** the platform is supported but no platform package is present in the installed dependency tree
- **THEN** the shim exits non-zero with the same actionable message
- **AND** no raw module-resolution stack trace is printed

### Requirement: Published Binaries Are The Released Binaries

Each platform package SHALL contain a binary taken from the artifacts the release build jobs already produced for the tag, repackaged rather than recompiled. The npm publish step SHALL NOT invoke a compiler.

#### Scenario: No compilation occurs during publication

- **WHEN** the npm publish job runs
- **THEN** it performs no Rust build and consumes only artifacts downloaded from the tag's build jobs

#### Scenario: The npm binary matches the release asset

- **WHEN** the `linux-x64` platform package's binary is compared with the `specforge-serve` binary inside that tag's Linux archive asset
- **THEN** the two are byte-identical

### Requirement: Publication Ordered After The GitHub Release

Because an npm publication cannot be reliably retracted, publication SHALL occur only after the tag's GitHub Release has been published successfully, and SHALL publish all five platform packages before publishing the wrapper that pins them. If any platform publication fails, the wrapper SHALL NOT be published. The publish step SHALL be re-runnable for an already-pushed tag, so that a transient registry failure does not require a new version.

#### Scenario: A failed release blocks publication

- **WHEN** the GitHub Release job for a tag fails
- **THEN** nothing is published to npm for that tag

#### Scenario: Platform packages publish before the wrapper

- **WHEN** publication runs for a tag
- **THEN** every platform package is published before the wrapper package

#### Scenario: A mid-flight failure leaves no broken wrapper

- **WHEN** one platform package fails to publish
- **THEN** the wrapper is not published
- **AND** the already-published platform packages remain unreferenced by any wrapper version, rather than a wrapper resolving to a version that does not exist

#### Scenario: Publication can be retried without a new tag

- **WHEN** publication fails for transient registry reasons and is re-run for the same tag
- **THEN** it can complete without pushing a new version tag

### Requirement: Every Published Package Carries The Tag Version

All six published packages SHALL carry the version derived from the release tag with its leading `v` stripped, identical to the version stamped into the workspace — see the *Version Derived From Tag* requirement in the `release-pipeline` capability. The version SHALL be applied to the generated manifests at publication time rather than read from a committed manifest, so the six versions cannot drift from one another or from the tag.

#### Scenario: All packages share the tag's version

- **WHEN** the tag `v0.19.0` triggers publication
- **THEN** the wrapper and all five platform packages are published at version `0.19.0`
- **AND** the wrapper's `optionalDependencies` pin each platform package to exactly `0.19.0`

#### Scenario: Versions are generated, not committed

- **WHEN** the repository is inspected at the tagged commit
- **THEN** no committed manifest for a published package records the release version, because the manifests are generated during publication

### Requirement: Prerelease Tags Publish Under A Non-Default Dist-Tag

A tag whose version carries a prerelease suffix SHALL be published under the `next` dist-tag rather than `latest`, so that installing the wrapper without an explicit version continues to resolve to the most recent stable release.

#### Scenario: A release candidate does not become the default install

- **WHEN** the tag `v0.20.0-rc.1` triggers publication
- **THEN** the packages are published under the `next` dist-tag
- **AND** `latest` continues to point at the most recent stable version

#### Scenario: A stable tag publishes to latest

- **WHEN** the tag `v0.20.0` triggers publication
- **THEN** the packages are published under the `latest` dist-tag

### Requirement: Publication Attaches Build Provenance

Publication SHALL attach npm build provenance, established from the release workflow's OIDC identity, to every package the release pipeline publishes. Provenance is a registry-verifiable statement about where and from which commit the package was built; it is not code signing, and it SHALL NOT be described as making the binaries signed — see the *Serve Binaries Unsigned* requirement in the `release-pipeline` capability.

The one-time placeholder publications described in the *Package Names Are Bootstrapped Before Automated Publication* requirement are necessarily unattested, because they are performed by hand rather than by the workflow. They SHALL be the only unattested versions of these packages.

#### Scenario: Published packages carry provenance

- **WHEN** a package published by the pipeline is inspected on the registry
- **THEN** it reports verified build provenance linking it to this repository and the workflow run that produced it

#### Scenario: Provenance does not imply a signed binary

- **WHEN** documentation or release notes describe the npm channel
- **THEN** they do not state or imply that the distributed executables are code-signed

### Requirement: Package Names Are Bootstrapped Before Automated Publication

npm stores a trusted-publisher configuration on a package that already exists, so a package's *first* publish cannot be performed by the release pipeline. Each of the published names SHALL therefore be established by a one-time manual placeholder publish, performed by a maintainer, after which that package's trusted publisher is configured and every subsequent publication is automated. Introducing a further platform package later SHALL require the same bootstrap for the new name before the pipeline can publish it.

A placeholder SHALL be published under an explicit bootstrap dist-tag, and SHALL be deprecated immediately after publication rather than only once a real release supersedes it. Requesting a non-default tag does not keep a placeholder out of the default install: npm assigns `latest` to a package's first published version regardless of the tag requested, so until a real release exists the placeholder is unavoidably what an unversioned install resolves to. Deprecation is the only available mitigation — it cannot move `latest`, but it makes such an install print a warning instead of silently delivering an empty package.

A placeholder SHALL be deprecated rather than unpublished — unpublishing every version of a name locks that name and would lock the project out of its own release.

#### Scenario: A placeholder takes the default tag despite the requested one

- **WHEN** a placeholder version is published to reserve a package name under a bootstrap dist-tag
- **THEN** npm additionally assigns `latest` to that version, because it is the package's only version
- **AND** an install that names no version resolves to the placeholder until a real release is published

#### Scenario: A placeholder is deprecated as soon as it is published

- **WHEN** a placeholder version has been published to reserve a package name
- **THEN** it is deprecated without waiting for the first real release
- **AND** an install that resolves to it prints a warning stating it is not a usable release

#### Scenario: The pipeline is never expected to perform a first publish

- **WHEN** the release pipeline publishes a package
- **THEN** that package already exists on the registry with a trusted publisher configured, so the publication authenticates by OIDC

#### Scenario: A newly added platform requires its own bootstrap

- **WHEN** a platform package for a new target is added to the published set
- **THEN** that name is bootstrapped and given a trusted publisher before the pipeline publishes it, rather than being expected to publish itself on the next release

#### Scenario: Placeholders are retired without releasing the name

- **WHEN** a real release supersedes a placeholder version
- **THEN** the placeholder is deprecated rather than unpublished, so the package name remains held by this project

### Requirement: Publication Uses An npm Client That Supports Trusted Publishing

The publication job SHALL run an npm client version that performs the OIDC token exchange. An older client does not attempt the exchange at all and fails as though the credentials were wrong, which is indistinguishable from a misconfigured trusted publisher and would surface only after the GitHub Release is public. The job SHALL therefore pin its client rather than inheriting whatever the runner image provides, and SHALL record the resolved version in its log.

#### Scenario: The job pins its npm client

- **WHEN** the publication job runs
- **THEN** it installs an npm version known to support trusted publishing, rather than relying on the version bundled with the runner's Node image

#### Scenario: The resolved client version is visible in the log

- **WHEN** the publication job runs
- **THEN** the npm version in use is printed before any publish is attempted

### Requirement: npm Installs Require No Quarantine Or Permission Workaround

Because files extracted by a package manager receive neither the macOS quarantine attribute nor the Windows Mark-of-the-Web, and because the executable bit is restored from the package tarball, the npm channel SHALL require no attribute-clearing, permission-changing, or Gatekeeper interaction to run the server.

#### Scenario: macOS npm install runs without clearing quarantine

- **WHEN** a user installs the wrapper on macOS and runs the `specforge-serve` bin
- **THEN** the server starts without any quarantine-clearing step and without a Gatekeeper prompt

#### Scenario: Extracted binary is already executable

- **WHEN** a platform package is installed on macOS or Linux
- **THEN** its binary's executable bit is set without a manual permission change

