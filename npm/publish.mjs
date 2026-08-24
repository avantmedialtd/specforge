#!/usr/bin/env node
// Publish the packages assembled by `build-packages.mjs`, in the order its plan
// specifies.
//
//   node npm/publish.mjs --dist <dir> [--dry-run]
//
// Two properties matter here, both because an npm publish cannot be reliably
// retracted:
//
//   Ordering — every platform package is published before the wrapper that pins
//   it. If a platform publish fails, this exits before the wrapper goes out, so
//   the failure leaves unreferenced orphan packages rather than a wrapper whose
//   pinned dependencies do not exist.
//
//   Idempotency — a package already published at this exact version is skipped
//   rather than retried. Re-running a partially-successful publish is the
//   normal recovery path for a transient registry failure, and without this it
//   would fail on the first already-published package and never reach the ones
//   that still need publishing.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

function parseArgs(argv) {
  const args = { dryRun: false };
  for (let i = 0; i < argv.length; i += 1) {
    const [flag, inline] = argv[i].split(/=(.*)/s);
    if (flag === "--dry-run") args.dryRun = true;
    else if (flag === "--dist") args.dist = inline ?? argv[++i];
  }
  if (!args.dist) {
    throw new Error("usage: publish.mjs --dist <dir> [--dry-run]");
  }
  return args;
}

const readJson = (file) => JSON.parse(readFileSync(file, "utf8"));

/**
 * Whether this exact version is already on the registry.
 *
 * `npm view` exits non-zero for an unpublished package or version, which is the
 * expected path on a first publish — so a failure here means "not published",
 * not "something went wrong".
 */
function alreadyPublished(name, version) {
  const result = spawnSync("npm", ["view", `${name}@${version}`, "version"], {
    encoding: "utf8",
  });
  return result.status === 0 && result.stdout.trim() === version;
}

function publish(dir, { distTag, dryRun }) {
  const args = [
    "publish",
    // Scoped packages default to restricted; without this the first publish of
    // each package would succeed as a private package nobody can install.
    "--access",
    "public",
    "--provenance",
    "--tag",
    distTag,
  ];
  if (dryRun) args.push("--dry-run");

  const result = spawnSync("npm", args, { cwd: dir, stdio: "inherit" });
  return result.status === 0;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const dist = path.resolve(args.dist);
  const plan = readJson(path.join(dist, "publish-plan.json"));

  console.log(
    `Publishing ${plan.order.length} packages for ${plan.version} ` +
      `under dist-tag "${plan.distTag}"${args.dryRun ? " (dry run)" : ""}`,
  );

  for (const key of plan.order) {
    const dir = path.join(dist, key);
    const manifestPath = path.join(dir, "package.json");
    if (!existsSync(manifestPath)) {
      throw new Error(`Plan names "${key}" but ${manifestPath} does not exist.`);
    }
    const { name, version } = readJson(manifestPath);

    if (!args.dryRun && alreadyPublished(name, version)) {
      console.log(`= ${name}@${version} already published, skipping`);
      continue;
    }

    console.log(`→ publishing ${name}@${version}`);
    if (!publish(dir, { distTag: plan.distTag, dryRun: args.dryRun })) {
      // Stop immediately. If this was a platform package the wrapper has not
      // been published yet and must not be.
      throw new Error(
        `Failed to publish ${name}@${version}.\n` +
          (key === "wrapper"
            ? `The platform packages are published; re-run this job to retry ` +
              `the wrapper. Already-published packages are skipped.`
            : `The wrapper was NOT published, so no released version points at ` +
              `an incomplete set. Re-run this job once the cause is fixed.`),
      );
    }
  }

  console.log(`Published ${plan.version} (${plan.distTag}).`);
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
