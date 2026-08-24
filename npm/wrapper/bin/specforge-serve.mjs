#!/usr/bin/env node
// The `specforge-serve` entry point of the `@avantmedia/specforge` wrapper.
//
// This file does as little as possible: find the binary that the matching
// platform package installed, hand it every argument untouched, and get out of
// the way. It deliberately does not parse, validate, or rewrite arguments —
// `specforge-serve --help` is the binary's own help, and a flag added to the
// server must never need a change here to reach it.

import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  exitCodeFor,
  locateBinary,
  supportedKeysFrom,
  unsupportedMessage,
} from "../lib/resolve.mjs";

const require = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));

const binary = locateBinary({
  platform: process.platform,
  arch: process.arch,
  resolvePackageJson: (specifier) => require.resolve(specifier),
});

if (binary === null) {
  const manifest = JSON.parse(
    readFileSync(path.join(here, "..", "package.json"), "utf8"),
  );
  console.error(
    unsupportedMessage(
      process.platform,
      process.arch,
      supportedKeysFrom(manifest.optionalDependencies),
    ),
  );
  process.exit(1);
}

// `stdio: "inherit"` hands the real terminal to the server, so its startup
// address and its non-loopback bind warning reach the user directly rather than
// through a buffer this process would have to relay.
const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  console.error(
    `specforge-serve: failed to execute ${binary}\n${result.error.message}`,
  );
  process.exit(1);
}

process.exit(exitCodeFor(result));
