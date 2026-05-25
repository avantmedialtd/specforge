# Add CI Pipeline

## Why

The repository has no automated checks. Lint, type, and test regressions reach `master` only when a human happens to run the right command locally, and the upcoming release pipeline needs a green-build baseline it can trust before producing user-facing artifacts. A single, fast PR-time pipeline closes that gap without taking on the cost or complexity of the cross-platform release matrix.

## What Changes

- Add `.github/workflows/ci.yml` triggered on every `push` and `pull_request`.
- Run four parallel jobs on `ubuntu-latest`:
  - **lint** — `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
  - **test** — `cargo test --workspace`.
  - **frontend** — `bun install --frozen-lockfile`, `bun run build` (which runs `tsc --noEmit` and then `vite build`).
  - **smoke** — `bun tauri build --debug --no-bundle` on `ubuntu-latest` to catch Tauri-shell breakage without paying the bundling cost.
- Cache the Rust target directory with `Swatinem/rust-cache` and the bun store with `oven-sh/setup-bun`'s built-in caching.
- Surface a single required status check per job so branch protection can enforce green CI before merge.

## Capabilities

### New Capabilities

- `continuous-integration`: PR-time validation pipeline that runs lint, test, frontend build, and a Tauri smoke build on every push and pull request, gated on a single CI runner OS (Linux) to keep iteration time and runner cost low.

### Modified Capabilities

<!-- None. CI is additive infrastructure; no existing capability's requirements change. -->

## Impact

- New file: `.github/workflows/ci.yml`. No application code changes.
- First run may surface latent `cargo fmt` / `cargo clippy` violations; those get fixed in-line with the CI change or in a precursor cleanup commit.
- A separate `add-release-pipeline` change will layer the tag-driven release matrix on top; this change deliberately scopes to PR validation only so the two land independently.
- Runner cost: zero — public repo, unlimited GitHub-hosted Linux minutes.
