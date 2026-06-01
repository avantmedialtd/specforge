# Tasks

## 1. Slot allocator

- [x] 1.1 Add `scripts/worktree-slot.ts` with a pure, unit-testable core: resolve repo root (`git rev-parse --show-toplevel`) and common git dir (`git rev-parse --path-format=absolute --git-common-dir`); registry path = `<common-git-dir>/specforge-worktree-slots.json`
- [x] 1.2 Implement `liveWorktrees()` via `git worktree list --porcelain`, and `pruneRegistry()` dropping entries whose path is no longer a live worktree
- [x] 1.3 Implement `resolveSlot(registry, livePaths, thisPath, mainPath)`: main checkout pinned to slot 0; reuse an existing recorded (non-zero, non-colliding) slot; otherwise the lowest free slot ≥1
- [x] 1.4 Persist the (pruned + updated) registry atomically (temp file + rename)
- [x] 1.5 Add unit tests for `resolveSlot` (lowest-free, reuse, freed-after-removal, main=0, drift heal, no-mutate) and the port formula (`scripts/worktree-slot.test.ts`, 16 tests passing)

## 2. Dev launcher

- [x] 2.1 Derive `vitePort = 1420 + slot*10` (`derivePort`)
- [x] 2.2 Exec `tauri dev --config '{"build":{"beforeDevCommand":"bun run dev -- --port <vitePort> --strictPort","devUrl":"http://localhost:<vitePort>"}}'` via `spawnSync` with inherited stdio (`buildTauriConfig` builds the JSON); added `--print`/`--dry-run` to resolve + show the command without writing or launching
- [x] 2.3 Print a one-line banner (slot, port, worktree path) before launching
- [x] 2.4 On a `strictPort` bind failure (non-zero exit), surface a clear message naming the slot's port (predictable ports over auto-retry)

## 3. Wire up the command

- [x] 3.1 Add `"wt:dev": "bun scripts/worktree-slot.ts"` to `package.json` scripts
- [x] 3.2 Confirm slot-0 → 1420 mapping (unit test `main checkout is pinned to slot 0` + `derivePort(0)=1420`; the persisted registry pins the main checkout to 0). Literal `bun run wt:dev` from the main checkout requires the script on `master`; verified by logic + the registry until merged.

## 4. Documentation

- [x] 4.1 Add a "Concurrent worktrees (dev slots)" subsection to `CLAUDE.md` plus a Commands-table row: run `bun run wt:dev` in a worktree; slot 0 = main = 1420; `port = 1420 + slot*10`; state shared by design
- [x] 4.2 Note the accepted limitation (concurrent instances co-write `activity.json` + window-state) and the forward-compat path (identifier override) for future state isolation
- [x] 4.3 (Optional) Documented in `CLAUDE.md` that `/into-worktree` should use `bun run wt:dev` rather than scouting a free port by hand (the command itself is a global skill outside this repo, so its file is not edited here)

## 5. Verify

- [x] 5.1 With the user's main app on 1420, ran `bun run wt:dev` in the magnus worktree: banner `slot 1 → http://localhost:1430`, vite bound 1430 (`ready in 482ms`), specforge binary launched — no `strictPort` collision
- [x] 5.2 The worktree window loaded and rendered the Dashboard with the same registered workspaces as the main checkout (Mushroom, SpecForge, Avant Media) — confirming the shared config dir; verified by screenshot
- [x] 5.3 Re-resolved with `bun run wt:dev --print` — still `slot 1 → 1430` (stable; reuses the recorded slot)
- [x] 5.4 Freed-slot reuse verified by the `resolveSlot` unit test `a removed worktree frees its slot for reuse` (prune drops the dead path, the next worktree reclaims the gap). Deterministic unit coverage rather than a live worktree add/remove.

## 6. Capture spec deltas

- [x] 6.1 Confirmed the `worktree-dev-slots` spec scenarios match the shipped behaviour (main=1420; lowest-free allocation; reuse; freed-on-removal; `1420 + slot*10`; shared config dir)
- [x] 6.2 `openspec validate worktree-dev-slots --strict` passes
