#!/usr/bin/env bun
/**
 * worktree-slot — run the SpecForge dev server on a per-worktree "slot".
 *
 * Every git worktree (and the main checkout) gets a stable slot number so its
 * dev server binds a non-colliding port:
 *
 *     vitePort = 1420 + slot * 10
 *
 * Slot 0 is the main checkout (today's default 1420). Slots are auto-allocated
 * — the lowest non-negative integer not held by a live worktree — and recorded
 * in a registry inside the repository's shared common git dir, so the human
 * never has to pick one. Removing a worktree frees its slot (its registry entry
 * is pruned on the next run, intersected against `git worktree list`).
 *
 * The slot is threaded into Tauri WITHOUT editing vite.config.ts or
 * tauri.conf.json: we launch `tauri dev` with an inline `--config` override
 * that sets the vite `--port` and a matching `devUrl`. (Tauri's devUrl is
 * static JSON and can't read an env var, so it must be rewritten to match the
 * port vite binds.)
 *
 * Usage:
 *   bun run wt:dev [--print]
 *
 *   --print   resolve and show the slot / port / launch command, then exit
 *             without starting the dev server (also accepts --dry-run).
 */
import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";

export const BASE_PORT = 1420;
export const SLOT_STRIDE = 10;
export const REGISTRY_BASENAME = "specforge-worktree-slots.json";

/** Map of worktree path -> assigned slot number. */
export type SlotRegistry = Record<string, number>;

/** vitePort = 1420 + slot*10. Slot 0 => 1420 (today's default). */
export function derivePort(slot: number): number {
  return BASE_PORT + slot * SLOT_STRIDE;
}

/** Lowest non-negative integer not present in `used`. */
export function lowestFreeSlot(used: Set<number>): number {
  let slot = 0;
  while (used.has(slot)) slot++;
  return slot;
}

/** Drop registry entries whose worktree path is no longer a live worktree. */
export function pruneRegistry(
  registry: SlotRegistry,
  livePaths: Iterable<string>,
): SlotRegistry {
  const live = new Set(livePaths);
  const next: SlotRegistry = {};
  for (const [path, slot] of Object.entries(registry)) {
    if (live.has(path)) next[path] = slot;
  }
  return next;
}

/**
 * Resolve this worktree's slot and return the registry to persist.
 *
 * Rules:
 *   - The main checkout is pinned to slot 0 on every run.
 *   - A non-main worktree reuses its recorded slot when that slot is still
 *     non-zero and free of collisions with other live worktrees.
 *   - Otherwise it is assigned the lowest free slot (>= 1, since slot 0 is
 *     reserved for the main checkout).
 *
 * The returned registry is pruned (dead worktrees removed), has the main
 * checkout pinned to 0, and records this worktree's slot.
 */
export function resolveSlot(args: {
  registry: SlotRegistry;
  livePaths: string[];
  thisPath: string;
  mainPath: string;
}): { slot: number; registry: SlotRegistry } {
  const next = pruneRegistry(args.registry, args.livePaths);
  // The main checkout always owns slot 0 — overwrite any drifted value.
  next[args.mainPath] = 0;

  if (args.thisPath === args.mainPath) {
    return { slot: 0, registry: next };
  }

  // Slots claimed by every OTHER live worktree (slot 0 is in here via main).
  const usedByOthers = new Set<number>();
  for (const [path, slot] of Object.entries(next)) {
    if (path !== args.thisPath) usedByOthers.add(slot);
  }

  const existing = next[args.thisPath];
  if (existing !== undefined && existing !== 0 && !usedByOthers.has(existing)) {
    return { slot: existing, registry: next };
  }

  const slot = lowestFreeSlot(usedByOthers);
  next[args.thisPath] = slot;
  return { slot, registry: next };
}

/** The inline Tauri `--config` override that routes vite + devUrl to `port`. */
export function buildTauriConfig(port: number): string {
  return JSON.stringify({
    build: {
      beforeDevCommand: `bun run dev -- --port ${port} --strictPort`,
      devUrl: `http://localhost:${port}`,
    },
  });
}

// ---------------------------------------------------------------------------
// I/O + git helpers (the impure shell around the pure core above; the unit
// tests exercise the pure functions and leave these to the integration run).
// ---------------------------------------------------------------------------

function git(args: string[]): string {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

/**
 * Ordered worktree paths from `git worktree list --porcelain`. Git always
 * lists the main worktree first, so `[0]` is the main checkout.
 */
function liveWorktrees(): string[] {
  return git(["worktree", "list", "--porcelain"])
    .split("\n")
    .filter((line) => line.startsWith("worktree "))
    .map((line) => line.slice("worktree ".length).trim())
    .filter(Boolean);
}

function readRegistry(path: string): SlotRegistry {
  if (!existsSync(path)) return {};
  try {
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      // Keep only well-formed `path -> non-negative integer` entries so a
      // hand-corrupted value can't poison allocation (it would otherwise count
      // as a claimed slot or NaN).
      const clean: SlotRegistry = {};
      for (const [key, value] of Object.entries(parsed)) {
        if (typeof value === "number" && Number.isInteger(value) && value >= 0) {
          clean[key] = value;
        }
      }
      return clean;
    }
  } catch {
    // A corrupt cache file must never block dev — fall back to empty.
  }
  return {};
}

/**
 * Atomic write (temp + rename) so a crash can't leave a half-written file. The
 * temp name is per-process so two worktrees running `wt:dev` at once each
 * rename their own complete file rather than clobbering a shared `.tmp`.
 */
function writeRegistry(path: string, registry: SlotRegistry): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.${process.pid}.tmp`;
  writeFileSync(tmp, `${JSON.stringify(registry, null, 2)}\n`);
  renameSync(tmp, path);
}

function main(): void {
  const printOnly = process.argv
    .slice(2)
    .some((a) => a === "--print" || a === "--dry-run");

  const thisPath = git(["rev-parse", "--show-toplevel"]);
  // --path-format=absolute keeps the registry path correct regardless of the
  // directory the command was invoked from (a bare --git-common-dir can be
  // returned relative to cwd from the main checkout).
  const commonDir = git([
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ]);
  const live = liveWorktrees();
  const mainPath = live[0] ?? thisPath;
  const registryPath = join(commonDir, REGISTRY_BASENAME);

  const { slot, registry } = resolveSlot({
    registry: readRegistry(registryPath),
    livePaths: live,
    thisPath,
    mainPath,
  });

  const port = derivePort(slot);
  const config = buildTauriConfig(port);

  console.log(`▸ SpecForge wt:dev — slot ${slot} → http://localhost:${port}`);
  console.log(`  worktree: ${thisPath}`);
  if (slot === 0) {
    console.log("  (main checkout · slot 0 · today's default port)");
  }

  if (printOnly) {
    // Dry run: show the launch command but write nothing and start nothing.
    console.log(`  tauri dev --config '${config}'`);
    return;
  }

  // Persist the allocation only on a real launch, then hand off to tauri dev.
  writeRegistry(registryPath, registry);
  const result = spawnSync("bun", ["tauri", "dev", "--config", config], {
    cwd: thisPath,
    stdio: "inherit",
  });

  if (result.error) {
    console.error(
      `wt:dev: failed to launch tauri dev — ${result.error.message}`,
    );
    process.exit(1);
  }
  if (result.signal) {
    // Killed by a signal (status is null in this case) — don't let it look
    // like a clean exit to anything chaining on our exit code.
    console.error(`wt:dev: dev server terminated by ${result.signal}.`);
    process.exit(1);
  }
  if (result.status !== 0) {
    // strictPort does not auto-bump (slot↔port stays deterministic), so a bind
    // failure surfaces here with the port the user can free or reassign.
    console.error(
      `wt:dev: dev server exited with status ${result.status}. ` +
        `Slot ${slot} maps to port ${port}; if the bind failed, another ` +
        `process is holding it.`,
    );
    process.exit(result.status ?? 1);
  }
}

if (import.meta.main) {
  try {
    main();
  } catch (err) {
    console.error(`wt:dev: ${(err as Error).message}`);
    process.exit(1);
  }
}
