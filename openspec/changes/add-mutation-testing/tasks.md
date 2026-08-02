# Tasks: Add Mutation Testing to the Headless Crates

## 1. Restore a green baseline

Mutation testing cannot start until `cargo test --workspace` passes: the tool
refuses to run against a failing baseline. It was red on macOS.

- [x] 1.1 In `crates/openspec-core/src/watcher.rs`, add a `#[doc(hidden)] pub mod recompute_gate` before `struct Inner`, providing `arm() -> Gate` and `pub(crate) rendezvous_if_armed()`. Document why it is not `#[cfg(test)]`-gated, mirroring `git::invocation_log` (`mutation-testing`: *Green baseline prerequisite*)
- [x] 1.2 Call `recompute_gate::rendezvous_if_armed()` in `refresh_aggregated_view_locked`, between the phase-1 gather block and the phase-2 git I/O, so the hook sits exactly on the boundary the invariant describes
- [x] 1.3 Delete the racing `concurrent_cache_write_is_not_blocked_by_an_in_flight_recompute` from `crates/openspec-core/tests/repo_monitor.rs`, drop the now-unused `AtomicBool`/`Ordering` import, and leave a comment recording where the coverage went and why
- [x] 1.4 Add `crates/openspec-core/tests/recompute_concurrency.rs` as its own target with exactly one test: arm the gate, park the recompute at the boundary, probe with a bare `remove_workspace` cache write on a bounded receive, release, assert (`mutation-testing`: *Green baseline prerequisite*)
- [x] 1.5 Verify the replacement can actually fail — temporarily hoist the `cache.read()` guard out of phase 1, confirm the test fails with its diagnostic rather than hanging, then revert

## 2. Mutation tooling

- [x] 2.1 Append `[profile.mutants]` (`inherits = "test"`, `debug = "none"`) to the root `Cargo.toml`, with a comment recording that `panic = "abort"` is release-only and therefore not inherited
- [x] 2.2 Add `.cargo/mutants.toml` with `exclude_globs` scoping out the three shell crates and the two untested files, plus `exclude_re`, `profile`, `test_workspace = false`, and the timeout floor — each exclusion carrying a written reason (`mutation-testing`: *Mutation-testing scope*, *Mutant timeout policy*)
- [x] 2.3 Add `mutants.out` and `mutants.out.old` to `.gitignore`, noting that this also keeps a previous run's output out of the next run's scratch tree
- [x] 2.4 Verify scope with `cargo mutants --list` — every mutant under the two in-scope `src/` trees, none from the shells, none from the excluded files (`mutation-testing`: *Mutation-testing scope*)
- [x] 2.5 Verify the build really is scoped: run a single-mutant shard and confirm the baseline builds in a scratch tree with no `dist/`, and that no shell crate is compiled (`mutation-testing`: *Mutation-testing scope*)
- [x] 2.6 Verify that a single-file `-f` filter narrows to that file rather than expanding to the full scope, and express the scope as exclusions if it does not — it did not narrow under an `examine_globs` allowlist, so the scope is written as exclusions (`mutation-testing`: *Mutation-testing scope*)

## 3. Continuous-integration gate

- [x] 3.1 Add `.github/workflows/mutants.yml` as a workflow separate from `ci.yml`, triggered on push plus `workflow_dispatch` with an optional base override (`mutation-testing`: *Changed-lines gate*)
- [x] 3.2 Implement diff-base resolution — merge-base for branch pushes, `push.before` for master pushes, merge-base fallback for first and force pushes — passing every untrusted value through `env:` rather than interpolating it into `run:` (`mutation-testing`: *Changed-lines gate*)
- [x] 3.3 Short-circuit on an empty diff before the toolchain or the mutation tool is installed, filtering the diff by pathspec to the two in-scope crates' sources (`mutation-testing`: *Changed-lines gate*)
- [x] 3.4 Set per-SHA concurrency on `master` and cancel-in-progress on feature branches, `timeout-minutes: 45`, a dedicated `rust-cache` key, artifact upload of `mutants.out`, and a job summary of survivors (`mutation-testing`: *Changed-lines gate*, *Mutant timeout policy*)
- [x] 3.5 Verify the base-resolution logic against all four cases — branch push, master fast-forward, first push with an all-zero `before`, and a force-push whose `before` no longer resolves

## 4. Baseline sweep and documentation

- [x] 4.1 Run the full sweep (`cargo mutants --no-shuffle -j4`) and record its date, scope, counts and wall-clock in a `README.md` table sourced from `mutants.out/outcomes.json` — 1453 mutants in 2h: 940 caught, 337 missed, 176 unviable, 0 timeouts (73.6% of viable) (`mutation-testing`: *Local invocation and recorded baseline*)
- [x] 4.2 Add command-table rows to `README.md` — reproduce the gate locally, list mutants without building, mutate a single file — plus a row in the CI table noting `mutants.yml` is a separate workflow (`mutation-testing`: *Local invocation and recorded baseline*)
- [x] 4.3 Add the local command to `CLAUDE.md`'s command table and a Conventions paragraph covering the gate, the written-reason exclusion rule, and the prohibition on suppressing the baseline check (`mutation-testing`: *Green baseline prerequisite*, *Local invocation and recorded baseline*)

## 5. Verification

- [x] 5.1 `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `bun run build` all green — `cargo test --workspace` passing on macOS is itself the fix from group 1
- [x] 5.2 `cargo test --profile mutants -p openspec-core` passes, and `cargo build` / `cargo clippy` are unaffected by the new profile
- [x] 5.3 Push the branch and confirm the **Mutants** workflow behaves on both paths: with in-scope Rust in the diff it resolved its base by merge-base, built its baseline in 19s+4s (proving the Tauri graph is never compiled) and tested 2 mutants in 39s; with no in-scope Rust it reported "Nothing to mutate" and skipped the toolchain, cache, tool install and mutation run entirely
- [x] 5.4 Confirm the gate actually gates — a deliberately unasserted function in `paths.rs` produced 10 mutants of which 9 survived, and `cargo mutants --in-diff` exited **2**, which fails the workflow step (no `continue-on-error`). Probe reverted

No manual UI smoke is required: this change touches no user-visible surface.
