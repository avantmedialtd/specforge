# Design — Worktree Dev Slots

## Context

SpecForge is a single Tauri desktop app: one vite dev server (`port: 1420, strictPort: true`), one Rust shell, one native window, one tray icon. There is no `.env`, no Docker, no database, and no single-instance plugin (the shell loads only `notification`, `autostart`, `window-state`, `dialog`). Two `bun tauri dev` instances can therefore coexist — but vite's `strictPort` makes the second one fail to bind 1420, and both resolve the same `app_config_dir()` (from identifier `com.avantmedia.specforge`), so they also share all on-disk state.

The goal, borrowed from `/Users/istvan/Developer/mushroom`, is for each worktree to run its dev app side-by-side with the main checkout. mushroom maps one `WORKTREE_SLOT` to `base + slot*100` across ~6 service ports plus a `COMPOSE_PROJECT_NAME`-namespaced database. SpecForge's topology collapses all of that to a single dev-server port — the one resource that actually collides in normal use.

## What needs isolating (the two axes)

```
AXIS A — dev PORT      vite 1420 (+HMR). strictPort ⇒ hard collision.   ← this change
AXIS B — STATE DIR     app_config_dir() from identifier ⇒ shared        ← deliberately NOT isolated
                       workspaces.json / settings.json / presentation.json /
                       activity.json / window-state.
AXIS C — VISUAL ID     two tray icons + two "SpecForge" windows.        ← out of scope
```

This change addresses **Axis A only**. Axis B is left shared on purpose (see Decision 1); Axis C is not attempted.

## Decisions

### Decision 1 — Port-only isolation; state stays shared

**Chosen:** Each slot changes only the dev-server port. All instances keep sharing the `com.avantmedia.specforge` config dir.

**Why:** The collision that blocks the documented worktree workflow is the port, and only the port. Sharing state is a *feature* here — a worktree's app opens already showing the main checkout's registered workspaces, with zero re-registration. The cost is that two live dev instances co-write `activity.json` and the window-state geometry; for a read-mostly viewer this is tolerable and was explicitly accepted.

**Alternatives considered and rejected:**
- *Port + isolated state* (override `identifier` per slot → separate config dir): true isolation, no races, but the worktree app starts empty and you must re-register workspaces. Too much friction for the common "quick visual check in a worktree" case.
- *Port + isolated + seeded* (isolate, then copy the main `workspaces.json` on first launch — the analogue of mushroom seeding `.env` from `.env.example`): the "best of both," but more machinery than the immediate pain warrants.
- *Port + isolated + visual marker* (the above plus a window-title suffix / tray tint): nice ergonomics, but scope creep relative to the port fix.

**Forward compatibility:** the launcher's inline `--config` is exactly where a future `identifier` override would be added, so escalating to isolated/seeded state later is an additive change to one command, not a redesign.

### Decision 2 — Auto-allocate the lowest free slot

**Chosen:** A registry maps worktree path → slot. The launcher prunes paths that are no longer live worktrees (`git worktree list --porcelain`), reuses this worktree's existing slot, else assigns the lowest non-negative integer not held by a live worktree. The main checkout is pinned to slot 0.

**Why:** SpecForge realistically runs 2–3 instances side-by-side, far fewer than mushroom. Auto-allocation makes the system zero-touch — better than mushroom's manual pick — and pinning main to 0 guarantees `1420` (today's default) never moves.

**Alternatives considered and rejected:**
- *Manual, like mushroom* (`wt:dev 1`, with collision detection but not prevention): most predictable, but adds ceremony for no benefit at SpecForge's scale.
- *Derive from name* (`hash(worktree-name) % N`): fully stateless, but hash collisions can put two worktrees on the same slot — the exact failure auto-allocation removes.

### Decision 3 — Registry lives in the common git directory

**Chosen:** Store the slot map at `<git-common-dir>/specforge-worktree-slots.json` (resolved via `git rev-parse --git-common-dir`).

**Why:** The common git dir is shared by the main checkout and every worktree automatically, and lives inside `.git`, so the file is never part of any working tree — no `.gitignore` entry, no risk of committing it. A single integer per worktree does not justify reintroducing mushroom's decentralized per-worktree store + sibling-scan.

### Decision 4 — Inline `--config` override threads the slot

**Chosen:** The launcher execs:
```
tauri dev --config '{"build":{
  "beforeDevCommand":"bun run dev -- --port <vitePort> --strictPort",
  "devUrl":"http://localhost:<vitePort>"
}}'
```

**Why:** Tauri's `devUrl` is static JSON and cannot read an env var, so a bare `WORKTREE_SLOT` env (mushroom's approach, which works there because its vite/server read the slot *in code*) is insufficient: `devUrl` must be rewritten to match whatever port vite binds. The inline override does this without a committed or generated config file and without touching `vite.config.ts` (the CLI `--port` overrides the hard-coded `port: 1420`; `--strictPort` is preserved). This was verified empirically while bootstrapping this worktree (vite bound 1430 via `bun run dev -- --port 1430 --strictPort` and the Tauri window loaded it).

### Decision 5 — Port math: `vitePort = 1420 + slot*10`

**Chosen:** Stride of 10, base 1420.

**Why:** SpecForge needs essentially one port per slot (HMR shares the server port locally; only the `TAURI_DEV_HOST` path uses a distinct HMR port, which still fits the 9-port gap). A stride of 10 keeps all slots in the conventional 1420-band and slot 0 stays exactly 1420. `×100` (mushroom parity) is the documented open question — its only advantage is cross-repo muscle memory.

## Risks / trade-offs

- **Shared-state races (accepted).** Concurrent dev instances co-write `activity.json` and window geometry. Documented; mitigated by the forward-compat path in Decision 1.
- **Unrelated process on a slot port.** The allocator does not probe port liveness; if something non-SpecForge holds the computed port, `strictPort` fails loudly rather than silently bumping (which would break slot↔port determinism). This is the intended behaviour (predictable ports over auto-retry) and is surfaced as an open question.
- **Registry drift.** If a worktree is deleted with plain `git worktree remove` (not via the app), its entry is pruned on the next `wt:dev` run via the live-worktree intersection, so freed slots are reclaimed automatically.
