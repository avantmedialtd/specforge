# Design — Add CI Pipeline

## Context

The repository has no `.github/workflows/` directory and no automated checks. All commits to date have relied on the author running `cargo test`, `bun run build`, and `bun tauri dev` locally. The project is small enough today that this has been workable, but two pressures push toward CI now:

1. A separate `add-release-pipeline` change will produce signed-eventually, user-facing artifacts on tag pushes. The release pipeline assumes the source tree it builds from has already passed lint + tests; without a CI gate, a broken `master` ships broken bundles.
2. The codebase is a Rust workspace (`openspec-core` + `specforge`) plus a TypeScript frontend plus a Tauri shell. Three toolchains, three failure modes, three things to break silently. PR-time validation catches regressions in whichever layer the author wasn't actively working on.

The Tauri app supports macOS, Windows, and Linux per the `tray-indicator` spec, but the vast majority of code is OS-agnostic — only `tray.rs`, `tray_icon.rs`, the autostart plugin, and a handful of platform-conditional blocks have OS-specific behavior. Linux runners catch the broad surface; the release pipeline's cross-platform matrix catches the rest.

## Goals / Non-Goals

**Goals:**

- Every push and pull request runs lint, test, frontend build, and a Tauri smoke build.
- Each check is a separate job so failures are independently visible (and re-runnable) in the GitHub UI.
- Median CI wall-clock time stays under ~5 minutes on warm caches.
- Zero runner cost — public repo, Linux-only.
- Branch protection can require all four jobs as gating status checks.

**Non-Goals:**

- Multi-OS PR validation. Cross-platform behavior is exercised by the release pipeline on tag pushes; replicating it on every PR would multiply minutes and wall-clock with low marginal value.
- Bundle production. PR CI runs `tauri build --debug --no-bundle`; full bundling (`.deb`, `.AppImage`, `.msi`, `.dmg`) is the release pipeline's job.
- Code-signing, notarization, artifact upload. All release-pipeline concerns.
- Auto-fixing `cargo fmt` or `cargo clippy --fix` violations. CI reports; humans fix.

## Decisions

### Linux-only for PR CI

Linux runners are unmetered on public repos and the fastest of the three. The OS-specific Rust code in `crates/specforge/` (tray APIs, autostart, window-state plugins) is wrapped in plugins that compile on Linux even when their runtime behavior is no-op or stubbed — `cargo build --workspace` on Linux exercises the entire compilation surface. The remaining true-platform-specific risk (notarization, macOS template glyph rendering, Windows installer authoring) only matters at bundle time, which is the release pipeline's concern.

**Alternative considered:** Full 3-OS matrix on PRs. Rejected: doubles wall-clock and triples the "which check is yellow" noise, with payoff only on the rare platform-specific edit. Easier to require contributors to run release-pipeline-equivalent builds on a draft tag when touching OS-specific code.

### Four jobs, not one

Split into `lint`, `test`, `frontend`, `smoke` rather than a single sequential script. Parallel execution finishes faster (each job ~1–3 min instead of ~5–8 min serial), and a failure in one doesn't mask another — a contributor who breaks both clippy and tests sees both at once rather than fixing clippy, repushing, and discovering the test failure on the next CI run.

The four-job split aligns with what a contributor would run locally: `cargo clippy`, `cargo test`, `bun run build`, `bun tauri build --debug`. Same mental model on both sides of the wall.

**Alternative considered:** One monolithic job that runs everything. Rejected for the masking and serial-time reasons above.

### Every Rust-compiling job also builds the frontend first

Discovered during implementation: `crates/specforge/src/lib.rs` calls `tauri::generate_context!()`, a proc macro that reads `tauri.conf.json` at compile time and validates that `frontendDist` (`../../dist`) exists on disk. Any `cargo` command that type-checks the `specforge` crate — including `cargo clippy --workspace --all-targets` and `cargo test --workspace` — fails with `The 'frontendDist' configuration is set to "../../dist" but this path doesn't exist` if the frontend hasn't been built.

This means lint, test, and smoke jobs all run `bun install --frozen-lockfile && bun run build` before any `cargo` invocation. The frontend job is the only one that stays bun-only.

**Cost:** ~5-15s per Rust job for the vite build (the `tsc --noEmit` half is essentially free at this codebase size). Bun install is ~1s with warm cache. Acceptable.

**Alternative considered:** A `prepare-frontend` job that runs `bun run build` once and uploads `dist/` as an artifact for lint/test/smoke to download. Rejected: serializes jobs that are currently parallel, and the artifact upload/download round-trip (~10-20s) eats most of the savings. Worth revisiting if the frontend build grows materially.

**Alternative considered:** Restructure `lib.rs` to load the Tauri context dynamically or stub `frontendDist` for non-bundle builds. Rejected: fights Tauri's intended invariants and would diverge from how every other Tauri 2 project is set up.

### Smoke build instead of full bundle

`bun tauri build --debug --no-bundle` compiles the Tauri shell against the frontend dist *without* packaging into `.AppImage` / `.deb`. This catches the most common Tauri-shell breakage (broken `tauri.conf.json`, missing capabilities, version-mismatched plugin), takes ~30s on warm cache, and skips the slow bundling step that the release pipeline is already going to do anyway.

**Alternative considered:** Skip smoke build entirely; rely on release-pipeline tag builds. Rejected because tag builds are infrequent (per-release, not per-PR), so a broken Tauri config could sit on `master` for days before being noticed.

### Caching: `Swatinem/rust-cache` + bun built-in

`Swatinem/rust-cache@v2` is the de facto standard for Rust GH Actions caching. It keys on `Cargo.lock` content, caches `~/.cargo/registry`, `~/.cargo/git`, and `target/` (with sensible exclusions for incremental artifacts that don't survive cache restoration well).

For bun, `oven-sh/setup-bun@v2` has `bun-version` pinning and an opt-in cache for `~/.bun/install/cache`.

GitHub's per-repo cache limit is 10 GB. A Rust `target/` for this workspace in debug mode is ~1–2 GB; bun deps are <100 MB. Four jobs × debug target × keyed-per-Cargo.lock = comfortable headroom.

**Alternative considered:** No caching, accept ~5–8 minute cold builds every run. Rejected: PR iteration speed matters more than the cache complexity.

### `bun install --frozen-lockfile`

CI must fail if `bun.lock` is out of date. Equivalent to `npm ci` or `pnpm install --frozen-lockfile`. Without it, CI silently regenerates the lockfile and the dependency tree drifts from what was reviewed.

## Risks / Trade-offs

- **Cache poisoning** → If `Swatinem/rust-cache` restores a stale `target/`, `cargo` is supposed to detect and rebuild affected crates. In practice this is robust, but if it ever fails, manual cache eviction via the GH UI is the escape hatch.
- **Linux-only blind spots** → A `cfg(target_os = "macos")` block can compile cleanly on Linux (it's gated out) and still ship broken. Mitigation: contributors editing OS-specific code should push a tag candidate first; the `tray.rs` / `tray_icon.rs` files are small enough that this is a known-narrow concern.
- **Frontend build runs in four jobs** → `bun run build` (`tsc --noEmit && vite build`) executes in `frontend`, `lint`, `test`, and implicitly in `smoke` (via `bun tauri build`'s `beforeBuildCommand`). Per-job cost is small (<15s) and bun's install cache keeps `bun install` near-instant, but it is duplicated work. See the "Every Rust-compiling job also builds the frontend first" decision for the rationale; revisit a shared `prepare-frontend` job if the frontend build grows materially.
- **First-run lint failures** → The repo has never been linted by `cargo fmt --check` or `cargo clippy -D warnings` in CI. Likely the first run surfaces a handful of violations. Fix in the same PR that introduces CI, or in a precursor cleanup commit.

## Migration Plan

1. Land this change as a single PR adding `.github/workflows/ci.yml`. The PR itself triggers the new workflow on `pull_request`, which is the smoke test.
2. Fix any latent lint/format violations surfaced by the first run.
3. After merge, configure branch protection on `master` to require `lint`, `test`, `frontend`, and `smoke` as status checks.

Rollback: delete `.github/workflows/ci.yml`. No state to migrate, no dependencies to unwind.

## Open Questions

None. All five decisions surfaced in conversation were resolved before drafting.
