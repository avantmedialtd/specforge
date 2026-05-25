## 1. Workflow scaffolding

- [x] 1.1 Create `.github/workflows/` directory at repo root.
- [x] 1.2 Create `.github/workflows/ci.yml` with `name: CI` and triggers on `push` and `pull_request` (no branch filter).
- [x] 1.3 Add `concurrency` group keyed on `${{ github.workflow }}-${{ github.ref }}` with `cancel-in-progress: true` so stale PR-CI runs are superseded by newer pushes.

## 2. Lint job

- [x] 2.1 Add `lint` job running on `ubuntu-latest`.
- [x] 2.2 Check out the repo via `actions/checkout@v4`.
- [x] 2.3 Install Rust stable via `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy`.
- [x] 2.4 Restore Rust cache via `Swatinem/rust-cache@v2`.
- [x] 2.5 Install bun + `bun install --frozen-lockfile` + `bun run build` so the frontend `dist/` exists for `tauri::generate_context!()` (see design.md "Every Rust-compiling job also builds the frontend first").
- [x] 2.6 Install Linux system dependencies required by Tauri 2 (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`) via `apt-get` so the `tauri` crate compiles for clippy.
- [x] 2.7 Run `cargo fmt --all -- --check`.
- [x] 2.8 Run `cargo clippy --workspace --all-targets -- -D warnings`.

## 3. Test job

- [x] 3.1 Add `test` job running on `ubuntu-latest`.
- [x] 3.2 Check out the repo via `actions/checkout@v4`.
- [x] 3.3 Install Rust stable via `dtolnay/rust-toolchain@stable`.
- [x] 3.4 Restore Rust cache via `Swatinem/rust-cache@v2` with `shared-key: "test"` so it doesn't collide with the lint job's cache slot.
- [x] 3.5 Install bun + `bun install --frozen-lockfile` + `bun run build` (same reason as 2.5).
- [x] 3.6 Install the Tauri Linux system deps (same set as 2.6).
- [x] 3.7 Run `cargo test --workspace`.

## 4. Frontend job

- [x] 4.1 Add `frontend` job running on `ubuntu-latest`.
- [x] 4.2 Check out the repo via `actions/checkout@v4`.
- [x] 4.3 Install bun via `oven-sh/setup-bun@v2` pinned to `bun-version: latest` (revisit pinning to a specific version once a `.bun-version` file exists).
- [x] 4.4 Run `bun install --frozen-lockfile`.
- [x] 4.5 Run `bun run build`.

## 5. Smoke job

- [x] 5.1 Add `smoke` job running on `ubuntu-latest`.
- [x] 5.2 Check out the repo via `actions/checkout@v4`.
- [x] 5.3 Install Rust stable via `dtolnay/rust-toolchain@stable`.
- [x] 5.4 Restore Rust cache via `Swatinem/rust-cache@v2` with `shared-key: "smoke"`.
- [x] 5.5 Install bun via `oven-sh/setup-bun@v2`.
- [x] 5.6 Install the Tauri Linux system dependencies (same set as 2.6).
- [x] 5.7 Run `bun install --frozen-lockfile` (the subsequent `tauri build` runs `bun run build` via `beforeBuildCommand`).
- [x] 5.8 Run `bun tauri build --debug --no-bundle`.

## 6. Latent-issue cleanup (surfaced during pre-flight)

- [x] 6.1 `cargo fmt --all` to fix formatting drift across 10 files under `crates/openspec-core/` and `crates/specforge/` (would have failed task 2.7 on first run).
- [x] 6.2 Fix `clippy::len_zero` warning in `crates/openspec-core/tests/self_write.rs:39` (`assert!(tracker.len() >= 1)` → `assert!(!tracker.is_empty())`).

## 7. Post-merge verification (requires GitHub UI / push to remote)

- [ ] 7.1 Push the branch and open a PR; confirm all four jobs run and their statuses appear on the PR.
- [ ] 7.2 Confirm cache hit on the second push to the same PR (lint job should drop from ~3 min to ~30 s).
- [ ] 7.3 After merge, configure GitHub branch protection on `master` to require `lint`, `test`, `frontend`, and `smoke` status checks.
