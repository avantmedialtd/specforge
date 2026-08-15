# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Product vs format naming

This project has two distinct names that must not collapse:

- **SpecForge** — the product (this desktop app). Used for the app name, window title, tray menu, bundle ID `com.avantmedia.specforge`, the Tauri crate `crates/specforge/`, and the npm package.
- **OpenSpec** — the *format* the app reads. Used for the on-disk layout (`openspec/`, `openspec/changes/`, `openspec/changes/archive/`), the `openspec-core` crate, workspace-validation errors, and file-picker prompts that ask the user for an OpenSpec workspace.

When editing user-visible copy, errors, dialogs, or path segments, pick deliberately. The `product-identity` spec at `openspec/specs/product-identity/spec.md` is the source of truth.

## Commands

Package manager is **bun**. Tauri dev/build commands are invoked through bun scripts (the Tauri config calls `bun run dev` / `bun run build`).

| Action | Command |
|---|---|
| Run the desktop app in dev mode | `bun tauri dev` |
| Run the dev app on this worktree's slot (side-by-side) | `bun run wt:dev` (see *Concurrent worktrees* below) |
| Run dev with WebView devtools auto-opened | `bun run tauri:devtools` (or set `SPECFORGE_OPEN_DEVTOOLS=1`) |
| Type-check + build frontend bundle | `bun run build` |
| Build production app bundle | `bun tauri build` |
| Run Rust tests (workspace) | `cargo test` |
| Run a single Rust integration test | `cargo test -p openspec-core --test parser` (replace `parser` with `cache` / `registry` / `watcher` / `self_write` / `recompute_concurrency`) |
| Run a single test by name | `cargo test -p openspec-core <name_substring>` |
| Mutation-test your changes (what CI gates on) | `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff` (install: `cargo install --locked cargo-mutants`) |
| List mutants in scope (instant, no build) | `cargo mutants --list` |

Frontend-only `vite` dev (`bun run dev`) exists but Tauri commands won't work without the Rust shell — use `bun tauri dev` for anything beyond CSS/markup tweaks.

### Concurrent worktrees (dev slots)

`bun tauri dev` binds vite on a fixed `port: 1420` with `strictPort: true`, so a second worktree's dev server collides with the main checkout's the moment both run. `bun run wt:dev` solves this by giving each worktree a stable **slot**.

State is **shared by design**: every instance still resolves the same `app_config_dir()` (from the `com.avantmedia.specforge` identifier), so a worktree's app opens showing the main checkout's registered workspaces. The trade-off is that concurrent instances co-write `activity.json` and window-state; a future change could isolate state by adding an `identifier` override in the same `--config`. Prefer `bun run wt:dev` over a hand-rolled port when running a worktree's app beside the main checkout (this is what `/into-worktree` should use instead of scouting a free port by hand).

## Architecture

Two-layer split: a headless Rust core (`openspec-core`) owns all state and filesystem logic; the Tauri shell (`specforge`) wraps it with a tray icon, commands, events, settings, and notifications. The React frontend is a pure consumer of those commands and events.

Per-layer detail lives next to the code it describes: `crates/CLAUDE.md` (Rust workspace) and `src/CLAUDE.md` (frontend).

### Window-lifecycle quirks

- The main window's **close button hides** rather than destroys (the watcher and tray must keep running). Cmd-Q, the "Quit SpecForge" tray item, or `app.exit(0)` are the only exit paths.
- On macOS, clicking the Dock icon when no windows are visible re-shows the main window (handled in the `RunEvent::Reopen` branch of `run()`).

## OpenSpec workflow

This repo dogfoods OpenSpec: each active change directory under `openspec/changes/` may contain `proposal.md`, `design.md`, `tasks.md`, and a `specs/<capability>/spec.md` subtree — the same four-artifact structure SpecForge browses for any registered workspace. When adding or modifying behaviour, check the relevant capability spec for the contract you're working against.

## Conventions

- **Start every task in a git worktree.** Before implementing a change *or* exploring an idea/spike, create and enter a dedicated worktree rather than working directly on `master`. This keeps the main checkout clean, lets parallel work and reviews proceed without collision, and makes throwaway explorations free to discard. Enter the worktree *before* running `openspec new change <name>`, so the scaffolded `openspec/changes/<id>/` files land in the right checkout instead of having to be moved across afterwards.
- Avoid `cd` in shell commands; use absolute paths or `cargo` / `bun` flags that target the right crate/package.
- Rust types crossing the IPC boundary use `#[serde(rename_all = "camelCase")]`. TypeScript mirrors live in `src/types.ts` — there's no codegen, so keep both sides matched.
- Don't introduce file watchers, registries, or parsers in the Tauri crate — that logic belongs in `openspec-core` so it stays testable from `cargo test`.
- For UI changes that need visual verification, start `bun tauri dev` yourself rather than asking the user to run it.
- **Mutation testing gates on changed lines.** `.github/workflows/mutants.yml` runs `cargo mutants --in-diff` against `origin/master` on every push, scoped to `openspec-core` + `openspec-app` (`.cargo/mutants.toml`). A survivor means a line you changed can be broken without any test noticing — add the assertion, or exclude the mutant in `.cargo/mutants.toml` with a written reason. Never reach for `--baseline=skip` to get past a failing test: with a red baseline every mutant's test run also fails, so every mutant reports as caught and the score is meaningless. Fix the test instead — and if its assertion depends on machine speed, make it deterministic rather than widening the margin (see `watcher.rs`'s `recompute_gate` for the pattern).
- After every `git push` to upstream, monitor the GitHub build (`gh run watch` or `gh run list --branch <branch>` then `gh run view <id> --log-failed` on failure) and report the outcome.
