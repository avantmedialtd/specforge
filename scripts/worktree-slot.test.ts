import { describe, expect, test } from "bun:test";
import {
  buildTauriConfig,
  derivePort,
  lowestFreeSlot,
  pruneRegistry,
  resolveSlot,
  type SlotRegistry,
} from "./worktree-slot.ts";

const MAIN = "/repo";
const A = "/repo/.claude/worktrees/a";
const B = "/repo/.claude/worktrees/b";
const C = "/repo/.claude/worktrees/c";

describe("derivePort", () => {
  test("slot 0 is the default 1420", () => {
    expect(derivePort(0)).toBe(1420);
  });

  test("each slot advances by 10", () => {
    expect(derivePort(1)).toBe(1430);
    expect(derivePort(2)).toBe(1440);
    expect(derivePort(7)).toBe(1490);
  });
});

describe("lowestFreeSlot", () => {
  test("empty set yields 0", () => {
    expect(lowestFreeSlot(new Set())).toBe(0);
  });

  test("contiguous from 0 yields the next", () => {
    expect(lowestFreeSlot(new Set([0]))).toBe(1);
    expect(lowestFreeSlot(new Set([0, 1, 2]))).toBe(3);
  });

  test("fills the lowest gap", () => {
    expect(lowestFreeSlot(new Set([0, 2]))).toBe(1);
    expect(lowestFreeSlot(new Set([1, 2]))).toBe(0);
  });
});

describe("pruneRegistry", () => {
  test("drops entries whose path is not live", () => {
    const reg: SlotRegistry = { [MAIN]: 0, [A]: 1, [B]: 2 };
    expect(pruneRegistry(reg, [MAIN, B])).toEqual({ [MAIN]: 0, [B]: 2 });
  });

  test("keeps everything when all live", () => {
    const reg: SlotRegistry = { [MAIN]: 0, [A]: 1 };
    expect(pruneRegistry(reg, [MAIN, A])).toEqual({ [MAIN]: 0, [A]: 1 });
  });
});

describe("resolveSlot", () => {
  test("main checkout is pinned to slot 0", () => {
    const { slot, registry } = resolveSlot({
      registry: {},
      livePaths: [MAIN],
      thisPath: MAIN,
      mainPath: MAIN,
    });
    expect(slot).toBe(0);
    expect(registry[MAIN]).toBe(0);
  });

  test("main is re-pinned to 0 even if the registry drifted", () => {
    const { slot, registry } = resolveSlot({
      registry: { [MAIN]: 5 },
      livePaths: [MAIN],
      thisPath: MAIN,
      mainPath: MAIN,
    });
    expect(slot).toBe(0);
    expect(registry[MAIN]).toBe(0);
  });

  test("a new worktree gets the lowest free non-zero slot", () => {
    const { slot } = resolveSlot({
      registry: { [MAIN]: 0, [A]: 1 },
      livePaths: [MAIN, A, B],
      thisPath: B,
      mainPath: MAIN,
    });
    expect(slot).toBe(2);
  });

  test("an existing recorded slot is reused even when a lower slot is free", () => {
    // B is recorded at 5 while slot 1 is free; only genuine reuse yields 5
    // (fresh lowest-free allocation would pick 1), so this guards the reuse
    // branch rather than coinciding with the fallback.
    const { slot } = resolveSlot({
      registry: { [MAIN]: 0, [B]: 5 },
      livePaths: [MAIN, B],
      thisPath: B,
      mainPath: MAIN,
    });
    expect(slot).toBe(5);
  });

  test("a removed worktree frees its slot for reuse", () => {
    // A (slot 1) is gone; C is new. C should reclaim the freed slot 1.
    const { slot, registry } = resolveSlot({
      registry: { [MAIN]: 0, [A]: 1, [B]: 2 },
      livePaths: [MAIN, B, C],
      thisPath: C,
      mainPath: MAIN,
    });
    expect(slot).toBe(1);
    expect(registry[A]).toBeUndefined();
  });

  test("a non-main worktree wrongly recorded as 0 is reallocated", () => {
    const { slot } = resolveSlot({
      registry: { [MAIN]: 0, [A]: 0 },
      livePaths: [MAIN, A],
      thisPath: A,
      mainPath: MAIN,
    });
    expect(slot).toBe(1);
  });

  test("does not mutate the input registry object", () => {
    const input: SlotRegistry = { [MAIN]: 0 };
    resolveSlot({
      registry: input,
      livePaths: [MAIN, A],
      thisPath: A,
      mainPath: MAIN,
    });
    expect(input).toEqual({ [MAIN]: 0 });
  });
});

describe("buildTauriConfig", () => {
  test("routes beforeDevCommand and devUrl to the port", () => {
    expect(JSON.parse(buildTauriConfig(1430))).toEqual({
      build: {
        beforeDevCommand: "bun run dev -- --port 1430 --strictPort",
        devUrl: "http://localhost:1430",
      },
    });
  });

  test("slot 0 maps to the default 1420 config", () => {
    expect(JSON.parse(buildTauriConfig(derivePort(0)))).toEqual({
      build: {
        beforeDevCommand: "bun run dev -- --port 1420 --strictPort",
        devUrl: "http://localhost:1420",
      },
    });
  });
});
