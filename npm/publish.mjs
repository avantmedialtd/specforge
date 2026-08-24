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
import { fileURLToPath } from "node:url";

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
 * Whether the registry already reports this exact version.
 *
 * `npm view` exits non-zero for an unpublished package or version, which is the
 * expected path on a first publish — so a non-zero exit means "not published",
 * not "something went wrong". A *spawn* failure is different and must not be
 * mistaken for "not published", so it is raised rather than swallowed.
 *
 * This answer is optimistic, never pessimistic: it can say "not published" for
 * something that was in fact just published, because packuments are served
 * through a cache and publish-time malware scanning delays visibility by
 * minutes. It cannot say "published" for something that was not. The publish
 * path below is what handles the optimistic direction.
 */
function alreadyPublished(name, version) {
  const result = spawnSync("npm", ["view", `${name}@${version}`, "version"], {
    encoding: "utf8",
  });
  if (result.error) {
    throw new Error(
      `Could not query the registry for ${name}@${version}: ${result.error.message}`,
    );
  }
  return result.status === 0 && result.stdout.trim() === version;
}

/**
 * Whether a failed publish failed *because the version is already there*.
 *
 * This is the other half of the re-run guarantee. Within the minutes-long
 * window where a just-published version is not yet visible to `npm view`, a
 * re-run reads "not published", tries to publish, and is rejected by the
 * registry. Treating that rejection as fatal would deadlock precisely the
 * recovery path this script exists to provide, so it is recognised and treated
 * as success instead.
 */
export function isPublishConflict(stderr) {
  return /EPUBLISHCONFLICT|cannot publish over/i.test(stderr ?? "");
}

function publish(dir, { distTag, dryRun }) {
  const args = [
    "publish",
    // Scoped packages default to restricted; without this the first publish of
    // each package would succeed as a private package nobody can install — and
    // on a free org plan it fails outright with a payment error.
    "--access",
    "public",
    "--tag",
    distTag,
  ];

  if (dryRun) {
    // No `--provenance` on a dry run. Provenance attests a real publish, and
    // generating it needs an OIDC token the runner only has when the job holds
    // `id-token: write`. Omitting it keeps `--dry-run` runnable anywhere — a
    // developer's laptop, or a CI job with no publishing rights at all.
    args.push("--dry-run");
  } else {
    args.push("--provenance");
  }

  // stderr is captured rather than inherited so a publish conflict can be
  // recognised, then echoed verbatim so nothing is hidden from the job log.
  const result = spawnSync("npm", args, {
    cwd: dir,
    stdio: ["inherit", "inherit", "pipe"],
    encoding: "utf8",
  });
  const stderr = result.stderr ?? "";
  if (stderr) process.stderr.write(stderr);

  return { ok: result.status === 0, stderr, error: result.error };
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
    const outcome = publish(dir, { distTag: plan.distTag, dryRun: args.dryRun });

    if (!outcome.ok) {
      // The registry rejecting this version as already present means a previous
      // attempt succeeded and the check above simply could not see it yet. That
      // is the re-run path working, not failing.
      if (isPublishConflict(outcome.stderr)) {
        console.log(
          `= ${name}@${version} already on the registry (publish conflict), continuing`,
        );
        continue;
      }
      // Stop immediately. If this was a platform package the wrapper has not
      // been published yet and must not be.
      throw new Error(
        `Failed to publish ${name}@${version}.` +
          (outcome.error ? `\n${outcome.error.message}` : "") +
          "\n" +
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

// Run only when invoked as a CLI. Without this guard, importing anything from
// this module — the unit tests import `isPublishConflict` — would execute
// main() and exit the process.
const invokedDirectly =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (invokedDirectly) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
