# continuous-integration Specification

## Purpose
TBD - created by archiving change add-ci-pipeline. Update Purpose after archive.
## Requirements
### Requirement: Pipeline Trigger

The continuous-integration pipeline SHALL execute on every `push` to any branch and on every `pull_request` opened, synchronised, or reopened against any branch.

#### Scenario: Push to feature branch triggers pipeline

- **WHEN** a commit is pushed to any branch in the repository
- **THEN** the CI pipeline begins executing within the GitHub Actions queue latency

#### Scenario: Pull request triggers pipeline

- **WHEN** a pull request is opened, has new commits pushed to its head branch, or is reopened
- **THEN** the CI pipeline executes against the merge commit between the PR head and its base

### Requirement: Rust Formatting Check

The pipeline SHALL fail if any Rust source file in the workspace does not conform to `cargo fmt` output.

#### Scenario: Misformatted Rust file fails the pipeline

- **WHEN** a Rust file in the workspace contains formatting that `cargo fmt` would change
- **THEN** the `lint` job exits non-zero and the pipeline is marked failed

### Requirement: Rust Lint Check

The pipeline SHALL fail if `cargo clippy --workspace --all-targets` emits any warning or error.

#### Scenario: Clippy warning fails the pipeline

- **WHEN** any crate in the workspace produces a clippy warning at any target
- **THEN** the `lint` job exits non-zero and the pipeline is marked failed

### Requirement: Rust Test Suite

The pipeline SHALL execute `cargo test --workspace` and fail if any test fails or any crate fails to compile in test configuration.

#### Scenario: Failing unit test fails the pipeline

- **WHEN** any `#[test]` function in any workspace crate panics or returns an error
- **THEN** the `test` job exits non-zero and the pipeline is marked failed

#### Scenario: Test-only compilation error fails the pipeline

- **WHEN** any workspace crate fails to compile under `cfg(test)`
- **THEN** the `test` job exits non-zero and the pipeline is marked failed

### Requirement: Frontend Type Check and Build

The pipeline SHALL execute `bun install --frozen-lockfile` followed by `bun run build`, and fail if either step exits non-zero.

#### Scenario: Lockfile drift fails the pipeline

- **WHEN** `bun.lock` is not in sync with `package.json`
- **THEN** `bun install --frozen-lockfile` exits non-zero and the pipeline is marked failed

#### Scenario: TypeScript error fails the pipeline

- **WHEN** any `.ts` or `.tsx` file in `src/` produces a `tsc --noEmit` error
- **THEN** the `frontend` job exits non-zero and the pipeline is marked failed

#### Scenario: Vite build error fails the pipeline

- **WHEN** `vite build` exits non-zero for any reason (missing import, plugin failure, syntax error)
- **THEN** the `frontend` job exits non-zero and the pipeline is marked failed

### Requirement: Tauri Smoke Build

The pipeline SHALL execute `bun tauri build --debug --no-bundle` on a Linux runner and fail if the Tauri shell does not compile against the frontend dist.

#### Scenario: Broken tauri.conf.json fails the pipeline

- **WHEN** `crates/specforge/tauri.conf.json` references a missing schema field, an undeclared plugin capability, or a malformed bundle target
- **THEN** the `smoke` job exits non-zero and the pipeline is marked failed

#### Scenario: Tauri plugin version mismatch fails the pipeline

- **WHEN** a `tauri-plugin-*` crate version in `Cargo.toml` is incompatible with the JS-side `@tauri-apps/plugin-*` version in `package.json`
- **THEN** the `smoke` job exits non-zero and the pipeline is marked failed

### Requirement: Parallel Job Execution

The pipeline SHALL execute the formatting check, lint check, test suite, frontend build, and Tauri smoke build as separate GitHub Actions jobs that run in parallel within the same workflow run.

#### Scenario: One failing job does not cancel others

- **WHEN** one job in the pipeline fails while others are still running
- **THEN** the other jobs continue to completion and report their independent status

#### Scenario: Each job is independently re-runnable

- **WHEN** a single job in a completed pipeline run fails
- **THEN** a user with write access can re-run that single job from the GitHub Actions UI without re-running successful jobs

### Requirement: Linux-Only Runner

All jobs in the continuous-integration pipeline SHALL run on `ubuntu-latest` GitHub-hosted runners.

#### Scenario: No macOS or Windows runner is used

- **WHEN** the CI pipeline executes
- **THEN** no job in the pipeline uses `macos-*` or `windows-*` runner labels

### Requirement: Dependency Caching

The pipeline SHALL cache the Rust toolchain output and the bun install cache between runs, keyed on the relevant lockfile content.

#### Scenario: Warm cache restores Rust target directory

- **WHEN** a pipeline run starts and `Cargo.lock` has not changed since the previous successful run
- **THEN** `Swatinem/rust-cache` restores `~/.cargo/registry`, `~/.cargo/git`, and `target/` from cache

#### Scenario: Warm cache restores bun dependencies

- **WHEN** a pipeline run starts and `bun.lock` has not changed since the previous successful run
- **THEN** the bun install cache is restored before `bun install --frozen-lockfile` runs

