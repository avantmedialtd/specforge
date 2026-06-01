# Worktree Dev Slots

## Why

SpecForge's frontend dev server binds `port: 1420` with `strictPort: true` (`vite.config.ts`), and the Tauri shell hard-codes `devUrl: "http://localhost:1420"` (`crates/specforge/tauri.conf.json`). The repo's own conventions tell every contributor to **start each task in a git worktree** and to **run `bun tauri dev` for any visual change** — but the moment a second worktree's dev server starts while the main checkout's is already running, vite cannot bind 1420 and `strictPort` aborts. There is no `.env` and no per-worktree configuration to fall back on, so today the only way to run a worktree's app side-by-side with the main checkout is to hand-write a one-off `tauri dev --config` override onto a manually-scouted free port. This exact dance was required to launch the worktree this proposal was authored in.

The sibling project at `/Users/istvan/Developer/mushroom` solves the equivalent problem with a **worktree slot system**: one `WORKTREE_SLOT` per worktree derives every port as `base + slot*100`, so each worktree "feels like its own isolated environment, browsable side by side." SpecForge wants the same ergonomics, scaled to its much simpler topology — a single desktop app with one dev server, no Docker, and no database.

This change brings a **port-only** slot system to SpecForge: each worktree is automatically assigned the lowest free slot, and a thin wrapper launches the dev server on that slot's port with no manual config. Slot 0 is the main checkout and keeps today's exact defaults (1420), so nothing changes for the primary workflow.

## What Changes

- **Add a slot allocator + dev launcher** invoked as `bun run wt:dev` (new `package.json` script backed by a new `scripts/worktree-slot.ts`). Running it in any worktree (or the main checkout) starts the SpecForge dev app on that worktree's slot port.
- **Auto-allocate the lowest free slot per worktree.** A small registry (a JSON map of worktree path → slot) is the source of truth. The allocator prunes entries for worktrees that no longer exist (`git worktree list`), reuses this worktree's slot if it already has one, otherwise assigns the lowest non-negative integer not currently claimed by a live worktree, and persists the result. The **main checkout is pinned to slot 0**. The registry lives in the repository's **common git directory** (`$(git rev-parse --git-common-dir)`), so it is shared across all worktrees automatically and is never part of any working tree (no `.gitignore` entry needed, no working-tree pollution).
- **Derive the port from the slot:** `vitePort = 1420 + slot*10` (slot 0 → 1420, slot 1 → 1430, slot 2 → 1440, …). The `×10` stride keeps every slot inside the familiar 1420-band; HMR rides the same port locally and has 9 ports of per-slot headroom for the `TAURI_DEV_HOST` case.
- **Launch via an inline `--config` override**, not a committed file: the wrapper execs `tauri dev --config '{"build":{"beforeDevCommand":"bun run dev -- --port <vitePort> --strictPort","devUrl":"http://localhost:<vitePort>"}}'`. This is required because Tauri's `devUrl` is static JSON that cannot read an env var — it is the one piece mushroom's in-code port derivation does not have to contend with — so *something* must rewrite `devUrl` to match the chosen vite port. The wrapper productizes exactly the manual override used to bootstrap this worktree.
- **Document the workflow** in `CLAUDE.md`, mirroring mushroom's "Concurrent Worktrees" section: run `bun run wt:dev` in a worktree; slot 0 = main = 1420; ports = `1420 + slot*10`.

### Explicitly out of scope (deliberate)

- **No state isolation.** Every instance continues to resolve `app_config_dir()` from the `com.avantmedia.specforge` identifier and therefore shares `workspaces.json`, `settings.json`, `presentation.json`, and `activity.json`. This is intentional: it means a worktree's app opens showing the same registered workspaces as the main app, with zero setup. The accepted consequence is that two running dev instances co-write `activity.json` and the window-state geometry in that shared directory. The wrapper is the natural seam where a future change could add an `identifier` override (relocating the whole config dir per slot) if state isolation is ever wanted — so this design stays forward-compatible without committing to it now.
- **No `.env` machinery.** Unlike mushroom (whose Docker interpolation forces literal `.env` values), SpecForge has no Docker and no secrets to preserve, so a single integer does not justify an `.env` file and an upsert script.
- **No Rust, IPC, frontend, or `vite.config.ts` changes.** The slot is threaded entirely through the launch command's `--config` override and the vite `--port` CLI flag.

## Capabilities

### New Capabilities

- `worktree-dev-slots`: The project provides a developer command that runs the SpecForge dev server on a per-worktree slot, auto-allocating the lowest free slot so multiple worktrees (and the main checkout) run their dev apps side-by-side without dev-server port collisions, while the main checkout keeps the default port.

## Impact

- **Tooling/build only.** New files: `scripts/worktree-slot.ts` (allocator + launcher) and a `wt:dev` script in `package.json`. Documentation update in `CLAUDE.md`.
- **No application behaviour change.** No edits to `openspec-core`, the Tauri shell crate, the IPC contract, Tauri command/event names, or any frontend code. `vite.config.ts` and `tauri.conf.json` are unchanged on disk — the port/devUrl are overridden only at launch time via `--config` and `--port`.
- **Slot 0 is the main checkout** and uses today's exact defaults, so the established `bun tauri dev` workflow is untouched; `wt:dev` is additive.
- **The existing `/into-worktree` command** can call `bun run wt:dev` in place of its current ad-hoc free-port scan, making slot allocation the single source of truth for the worktree dev port.
- **Known limitation (accepted):** shared config dir means concurrent dev instances race on `activity.json` and window geometry. Documented, with the identifier-override upgrade path noted.

## Open Questions

- **Port stride: `×10` vs `×100`.** This proposal uses `×10` to stay in the 1420-band (SpecForge needs only ~1 port per slot). `×100` would mirror mushroom exactly and give cross-repo muscle memory ("slot 1 = +100 everywhere"), at the cost of leaving the 1420-band. Confirm `×10` before landing.
- **Stale-port robustness.** The allocator trusts the slot→port map for determinism; it does not probe whether the computed port is *actually* free of unrelated processes. If a non-SpecForge process holds a slot port, `strictPort` will fail with a clear error rather than silently bumping the slot (which would break slot↔port determinism). Confirm this is the desired trade (predictable ports over auto-retry).
