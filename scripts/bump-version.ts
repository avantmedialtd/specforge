#!/usr/bin/env bun
/**
 * Convenient version bump for SpecForge.
 *
 * Releases are tag-driven: pushing a `v*` tag triggers the release workflow,
 * which stamps the version from the tag into Cargo.toml and tauri.conf.json
 * (only inside the workflow checkout — never committed back). So the tracked
 * files are NOT the version source; the latest `v*` tag is. This script derives
 * the current version from that tag, computes the next one, and creates the
 * matching annotated tag locally. It edits no files and does not push.
 *
 * Usage:
 *   bun run version <patch|minor|major|x.y.z> [--dry-run] [--force]
 */
import { execFileSync } from "node:child_process";

type SemVer = [number, number, number];

function git(args: string[]): string {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

function parseSemVer(value: string): SemVer | null {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(value);
  if (!m) return null;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

function compare(a: SemVer, b: SemVer): number {
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

function format([major, minor, patch]: SemVer): string {
  return `${major}.${minor}.${patch}`;
}

function currentVersion(): SemVer {
  const tags = git(["tag", "-l", "v*"])
    .split("\n")
    .map((t) => t.trim())
    .filter(Boolean)
    .map((t) => parseSemVer(t.replace(/^v/, "")))
    .filter((v): v is SemVer => v !== null);
  if (tags.length === 0) return [0, 0, 0];
  tags.sort(compare);
  return tags[tags.length - 1];
}

function nextVersion(current: SemVer, bump: string): SemVer {
  const [major, minor, patch] = current;
  switch (bump) {
    case "major":
      return [major + 1, 0, 0];
    case "minor":
      return [major, minor + 1, 0];
    case "patch":
      return [major, minor, patch + 1];
    default: {
      const explicit = parseSemVer(bump);
      if (!explicit) {
        fail(
          `Invalid argument "${bump}". Expected patch | minor | major | x.y.z.`,
        );
      }
      return explicit;
    }
  }
}

function fail(message: string): never {
  console.error(`error: ${message}`);
  process.exit(1);
}

function usage(): never {
  console.error(
    "usage: bun run version <patch|minor|major|x.y.z> [--dry-run] [--force]",
  );
  process.exit(1);
}

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const force = args.includes("--force");
const positional = args.filter((a) => !a.startsWith("--"));

if (positional.length !== 1) usage();
const bump = positional[0];

const current = currentVersion();
const next = nextVersion(current, bump);

// For explicit versions, reject downgrades/equal unless --force.
if (parseSemVer(bump) && compare(next, current) <= 0 && !force) {
  fail(
    `Refusing to set v${format(next)} — not greater than current v${format(
      current,
    )}. Use --force to override.`,
  );
}

const tag = `v${format(next)}`;

// Refuse if the tag already exists.
try {
  execFileSync("git", ["rev-parse", "-q", "--verify", `refs/tags/${tag}`], {
    stdio: "ignore",
  });
  fail(`Tag ${tag} already exists.`);
} catch {
  // Non-zero exit means the tag does not exist — good.
}

console.log(`v${format(current)} -> ${tag}`);

if (dryRun) {
  console.log("(dry run — no tag created)");
  process.exit(0);
}

// Warn (don't block) on a dirty tree: the tag points at HEAD, so uncommitted
// work will not be part of the release.
const dirty = git(["status", "--porcelain"]);
if (dirty) {
  console.warn(
    "warning: working tree is dirty — the tag points at HEAD and uncommitted changes won't be released.",
  );
}

execFileSync("git", ["tag", "-a", tag, "-m", tag]);
console.log(`Created tag ${tag}. Push it to release:`);
console.log(`  git push origin ${tag}`);
