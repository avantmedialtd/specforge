# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Product vs format naming

This project has two distinct names that must not collapse:

- **SpecForge** — the product (this desktop app). Used for the app name, window title, tray menu, bundle ID `com.avantmedia.specforge`, the Tauri crate `crates/specforge/`, and the published npm package `@avantmedia/specforge`. That package is **scoped**: the unscoped `specforge` on the public registry belongs to an unrelated project, so the scope is what preserves the product name as the published identity.
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
| Run Rust tests (workspace) | `bun install && bun run build` **once per fresh worktree**, then `cargo test` |
| Run a single Rust integration test | `cargo test -p openspec-core --test <target>` — `ls crates/openspec-core/tests/` for the live set |
| Run a single test by name | `cargo test -p openspec-core <name_substring>` |
| Mutation-test your changes (what CI gates on) | `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff` (install: `cargo install --locked cargo-mutants`) |
| List mutants in scope (instant, no build) | `cargo mutants --list` |
| Run the marketing site in dev mode | `bun run site:dev` (see *The marketing site* below) |
| Build the marketing site | `bun run site:build` |
| Run the marketing site's e2e suite | `bun run site:test` |

**`cargo test` fails workspace-wide in a fresh worktree until `dist/` exists.** It is gitignored, and both Tauri's `generate_context!` and specforge-web's `RustEmbed` need it at compile time — the failure surfaces as an opaque proc-macro error about a missing directory, not as a missing bundle. A *stale* `dist/` is the other half of the trap: debug builds of `specforge-web` read it from disk per request, so re-run `bun run build` before trusting a UI check.

`bun run dev` is Vite + HMR on 1420 and proxies `/api` (invoke + SSE) to a **running** `specforge-serve` (default port 4317, override with `SPECFORGE_WEB_PORT`). Start that server first and commands and events work in a plain browser tab with no Tauri shell; without it, `bun run dev` is markup/CSS only. Use `bun tauri dev` when the native shell itself — tray, dock badge, notifications, window lifecycle — is what you are verifying.

### Concurrent worktrees (dev slots)

`bun tauri dev` binds vite on a fixed `port: 1420` with `strictPort: true`, so a second worktree's dev server collides with the main checkout's the moment both run. `bun run wt:dev` solves this by giving each worktree a stable **slot**.

State is **shared by design**: every instance resolves the same `config_dir()` (from the `com.avantmedia.specforge` identifier), so a worktree's app opens showing the main checkout's registered workspaces. The trade-off is that concurrent instances co-write `activity.json` and window-state; a future change could isolate state by adding an `identifier` override in the same `--config`. Any worktree-setup flow should use `bun run wt:dev` rather than scouting a free port by hand.

### The marketing site

`site/` is the Vike + React static site published at `specforge.avantmedia.uk`. It documents this app's binaries, flags and install routes, which is why it lives here rather than in the studio monorepo it moved out of.

It is **deliberately isolated**: its own `package.json`, `bun.lock` and `node_modules`, and **not** a member of any workspace glob. The root `bun install`, the root `tsc` (`include: ["src"]`) and the root Vite build never touch it, and it resolves its own React/Vite versions rather than the desktop app's.

Three things bite if you forget them:

- **Every site command must run with the working directory set to `site/`.** The sitemap and search-index Vite plugins resolve `<cwd>/pages` and write `<cwd>/public`. The root `site:*` scripts all go through `bun run --cwd site` for exactly this reason.
- **The root `bunfig.toml` is load-bearing.** `bun test` matches `*.spec.ts` as well as `*.test.ts`, so without `[test] pathIgnorePatterns` the root test job collects the site's Playwright specs and dies on the first `test.describe()`. If root test discovery ever changes from 23 files, check that file first.
- **`site/site-kit/` is a vendored fork**, not a dependency — see its README for the source commit and the `diff -r` drift check. Do not reintroduce a barrel that re-exports a `Layout`; the upstream ones reach into a design system that was deliberately left behind.

Publishing is `.github/workflows/site.yml`, path-filtered to `site/**` and separate from `ci.yml` on purpose. It stays inert until the `SITE_DEPLOY_ROLE_ARN` repository variable is set, and dry-runs until `SITE_DEPLOY_MODE` is `live`.

## Architecture

Five crates in two library layers and three frontends:

- **`openspec-core`** — pure primitives: parsing, the registry, the cache, filesystem watching, git, and the data shapes that cross the IPC boundary.
- **`openspec-app`** — the headless application service. Owns the stateful brain neither frontend should duplicate: `AppService`, the file-backed `SettingsStore`, dashboard assembly, watcher lifecycle, the shared config directory, and every event name and payload shape.
- **`specforge`** (Tauri desktop), **`specforge-tui`** (ratatui terminal), **`specforge-web`** (browser, standalone binary `specforge-serve`) — three thin frontends over the same service.

A user-visible change usually has to land in more than one frontend. Logic that ends up inside a frontend crate is invisible to the other two.

`specforge-web` is also a **distribution channel**: its `specforge-serve` binary ships on npm as `@avantmedia/specforge` (a wrapper) plus five `@avantmedia/specforge-<platform>` packages, assembled from the `npm/` tree and published by `release.yml`'s `publish-npm` job on every `v*` tag — a prerelease tag lands on the `next` dist-tag, a final one on `latest`. Changing the binary's CLI surface or the supported platform matrix means updating `npm/packaging.mjs` in the same change — `npm/README.md` is the maintainer runbook.

Per-layer detail lives next to the code it describes: `crates/CLAUDE.md` (Rust workspace) and `src/CLAUDE.md` (frontend, including the four places a new command must be registered).

### Window-lifecycle quirks

- The main window's **close button hides** rather than destroys (the watcher and tray must keep running). Cmd-Q, the "Quit SpecForge" tray item, or `app.exit(0)` are the only exit paths.
- On macOS, clicking the Dock icon when no windows are visible re-shows the main window (handled in the `RunEvent::Reopen` branch of `run()`).

## OpenSpec workflow

This repo dogfoods OpenSpec: each active change directory under `openspec/changes/` may contain `proposal.md`, `design.md`, `tasks.md`, and a `specs/<capability>/spec.md` subtree — the same four-artifact structure SpecForge browses for any registered workspace. When adding or modifying behaviour, check the relevant capability spec for the contract you're working against.

## Conventions

- **Start every task in a git worktree.** Before implementing a change *or* exploring an idea/spike, create and enter a dedicated worktree rather than working directly on `master`. This keeps the main checkout clean, lets parallel work and reviews proceed without collision, and makes throwaway explorations free to discard. Enter the worktree *before* running `openspec new change <name>`, so the scaffolded `openspec/changes/<id>/` files land in the right checkout instead of having to be moved across afterwards.
- Avoid `cd` in shell commands; use absolute paths or `cargo` / `bun` flags that target the right crate/package.
- Rust types crossing the IPC boundary use `#[serde(rename_all = "camelCase")]`. TypeScript mirrors live in `src/types.ts` — there's no codegen, so keep both sides matched.
- **Keep the frontend crates thin.** Parsers, registries, caches and watchers belong in `openspec-core` so they stay testable from `cargo test`; anything stateful that more than one frontend needs — settings, service orchestration, dashboard assembly, quota — belongs in `openspec-app`. `crates/specforge/src/commands.rs` should only deserialize args and call `AppService` / `SettingsStore`.
- For UI changes that need visual verification, start the app yourself rather than asking the user to run it. The browser loop (`specforge-serve` + `bun run dev`) is the lighter default; reach for `bun tauri dev` when the native shell is what's under test.
- **Mutation testing gates on changed lines.** `.github/workflows/mutants.yml` runs `cargo mutants --in-diff` against `origin/master` on every push, scoped to `openspec-core` + `openspec-app` (`.cargo/mutants.toml`). A survivor means a line you changed can be broken without any test noticing — add the assertion, or exclude the mutant in `.cargo/mutants.toml` with a written reason. Never reach for `--baseline=skip` to get past a failing test: with a red baseline every mutant's test run also fails, so every mutant reports as caught and the score is meaningless. Fix the test instead — and if its assertion depends on machine speed, make it deterministic rather than widening the margin (see `watcher.rs`'s `recompute_gate` for the pattern).
- **Only those two crates are gated.** A diff touching only `crates/specforge/`, `crates/specforge-tui/` or `crates/specforge-web/` short-circuits the Mutants job — it reports green in seconds without running anything. Green there means "not run", not "covered"; cover those crates with ordinary tests.
- After every `git push` to upstream, monitor the GitHub build (`gh run watch` or `gh run list --branch <branch>` then `gh run view <id> --log-failed` on failure) and report the outcome.
