# Tasks

## 1. Slot allocator

- [ ] 1.1 Add `scripts/worktree-slot.ts` with a pure, unit-testable core: resolve repo root (`git rev-parse --show-toplevel`) and common git dir (`git rev-parse --git-common-dir`); registry path = `<common-git-dir>/specforge-worktree-slots.json`
- [ ] 1.2 Implement `liveWorktrees()` via `git worktree list --porcelain`, and prune registry entries whose path is no longer a live worktree
- [ ] 1.3 Implement `allocateSlot(registry, thisWorktreePath, mainCheckoutPath)`: main checkout pinned to slot 0; reuse an existing recorded slot; otherwise return the lowest non-negative integer not held by a live worktree
- [ ] 1.4 Persist the (pruned + updated) registry atomically
- [ ] 1.5 Add unit tests for `allocateSlot` (lowest-free, reuse, freed-after-removal, main=0) and the port formula

## 2. Dev launcher

- [ ] 2.1 Derive `vitePort = 1420 + slot*10`
- [ ] 2.2 Exec `tauri dev --config '{"build":{"beforeDevCommand":"bun run dev -- --port <vitePort> --strictPort","devUrl":"http://localhost:<vitePort>"}}'`, inheriting stdio so it behaves like `bun tauri dev`
- [ ] 2.3 Print a one-line banner (slot, port, worktree path) before launching
- [ ] 2.4 On a `strictPort` bind failure, surface a clear message naming the busy port (predictable ports over auto-retry)

## 3. Wire up the command

- [ ] 3.1 Add `"wt:dev": "bun scripts/worktree-slot.ts"` to `package.json` scripts
- [ ] 3.2 Confirm `bun run wt:dev` in the main checkout binds 1420 and behaves identically to `bun tauri dev` today

## 4. Documentation

- [ ] 4.1 Add a "Concurrent worktrees" subsection to `CLAUDE.md`: run `bun run wt:dev` in a worktree; slot 0 = main = 1420; `port = 1420 + slot*10`; state is shared by design
- [ ] 4.2 Note the accepted limitation (concurrent instances co-write `activity.json` + window-state) and the forward-compat path (identifier override) for future state isolation
- [ ] 4.3 (Optional) Point the `/into-worktree` command at `bun run wt:dev` instead of its ad-hoc free-port scan

## 5. Verify

- [ ] 5.1 With the main checkout's dev server running on 1420, run `bun run wt:dev` in a second worktree and confirm it launches on the next slot port with no `strictPort` collision
- [ ] 5.2 Confirm the worktree app loads (Tauri window appears) and shows the same registered workspaces as the main checkout
- [ ] 5.3 Re-run `bun run wt:dev` in the same worktree and confirm the slot/port are stable
- [ ] 5.4 Remove a throwaway worktree, run `wt:dev` in a fresh one, and confirm the freed slot is reused

## 6. Capture spec deltas

- [ ] 6.1 Confirm the `worktree-dev-slots` spec scenarios match the shipped behaviour (main=1420; lowest-free allocation; reuse; freed-on-removal; `1420 + slot*10`; shared config dir)
- [ ] 6.2 `openspec validate worktree-dev-slots --strict` passes
