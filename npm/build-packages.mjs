#!/usr/bin/env node
// Assemble every npm package for a release from binaries the build jobs already
// produced.
//
//   node npm/build-packages.mjs --version 0.19.0 --binaries <dir> --out <dir>
//
// `--binaries` is a directory holding one subdirectory per platform key, each
// containing that platform's executable:
//
//   <binaries>/darwin-arm64/specforge-serve
//   <binaries>/linux-x64/specforge-serve
//   <binaries>/win32-x64/specforge-serve.exe
//
// This script compiles nothing. The binaries it copies are the same ones
// attached to the GitHub Release, so the two channels ship identical bytes.
//
// Nothing it writes is committed: the manifests are generated per release so
// six version fields cannot drift from one another or from the tag.

import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  PLATFORMS,
  assertVersion,
  binaryNameFor,
  distTagFor,
  installSpecFor,
  platformManifest,
  platformPackageName,
  publishOrder,
  wrapperManifest,
  wrapperPackageName,
} from "./packaging.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const [flag, inline] = argv[i].split(/=(.*)/s);
    if (!flag.startsWith("--")) continue;
    const name = flag.slice(2);
    args[name] = inline ?? argv[++i];
  }
  for (const required of ["version", "binaries", "out"]) {
    if (!args[required]) {
      throw new Error(
        `Missing --${required}.\n` +
          `usage: build-packages.mjs --version <x.y.z> --binaries <dir> --out <dir>`,
      );
    }
  }
  return args;
}

const writeJson = (file, value) =>
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);

/**
 * Copy only `.mjs` sources into the wrapper package.
 *
 * The wrapper's `files` allowlist names whole directories, and `lib/` also
 * holds the shim's unit tests in the repository. Filtering by extension here is
 * what keeps a `.test.ts` out of the published tarball.
 */
function copySources(fromDir, toDir) {
  mkdirSync(toDir, { recursive: true });
  for (const entry of readdirSync(fromDir)) {
    if (!entry.endsWith(".mjs")) continue;
    copyFileSync(path.join(fromDir, entry), path.join(toDir, entry));
  }
}

function platformReadme(platform, version) {
  return [
    `# ${platformPackageName(platform.key)}`,
    "",
    `The SpecForge headless web server binary for ${platform.label}, version ${version}.`,
    "",
    `This package is installed automatically as an optional dependency of`,
    `[\`${wrapperPackageName()}\`](https://www.npmjs.com/package/${wrapperPackageName()})`,
    `on matching machines. Install that instead — there is no reason to depend`,
    `on this package directly.`,
    "",
  ].join("\n");
}

function wrapperReadme(version) {
  // A prerelease publishes to the `next` dist-tag, so the bare package name
  // resolves to `latest` — which is NOT this version. Pin the install commands
  // to the exact version instead. This matters more than it looks: a published
  // version's README can never be corrected, because that version can never be
  // republished. Getting it wrong leaves a permanently broken command on the
  // package page.
  const spec = installSpecFor(version);

  return `# SpecForge — headless web server

Browse [OpenSpec](https://github.com/Fission-AI/OpenSpec) workspaces in your
browser. This package ships \`specforge-serve\`, the standalone SpecForge web
server: one binary with the UI embedded, no separate assets to deploy.

Version ${version}.

## Run it

\`\`\`bash
# In any directory containing an openspec/ workspace
npx ${spec}
\`\`\`

It binds \`127.0.0.1:4317\` and serves the UI there.

For a server you intend to leave running — a remote dev box or a homelab
machine — install it rather than using \`npx\`, which re-resolves the package on
every invocation:

\`\`\`bash
npm install -g ${spec}
specforge-serve
\`\`\`

## Options

\`\`\`
--bind <addr>   interface to bind [env: SPECFORGE_WEB_BIND] (default: 127.0.0.1)
--port <port>   port to listen on  [env: SPECFORGE_WEB_PORT] (default: 4317)
--help          print the full reference and exit
\`\`\`

**\`--bind\` on a non-loopback interface publishes the workspace-reading API to
everyone who can reach the port, without authentication.** The default is
loopback; only widen it on a network you trust.

## Platforms

macOS (Apple Silicon and Intel), Linux (x64 and arm64), and Windows (x64). The
matching binary is selected automatically at install time — only one is
downloaded.

The Linux binaries are statically linked against musl, so they run on musl
distributions such as Alpine as well as on glibc distributions, including older
long-term-support releases.

## Notes

Installing through npm needs no macOS quarantine step: files extracted by a
package manager are not flagged the way a browser download is. The binaries are
**not** code-signed — packages carry npm build provenance, which attests where
they were built, not who authored them.

The desktop application and the terminal UI are distributed separately, from
[GitHub Releases](https://github.com/avantmedialtd/specforge/releases/latest).
`;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const version = assertVersion(args.version);
  const binaries = path.resolve(args.binaries);
  const out = path.resolve(args.out);

  if (existsSync(out)) rmSync(out, { recursive: true });
  mkdirSync(out, { recursive: true });

  for (const platform of PLATFORMS) {
    const binary = binaryNameFor(platform.os);
    const source = path.join(binaries, platform.key, binary);
    if (!existsSync(source)) {
      throw new Error(
        `Missing binary for ${platform.key}: expected ${source}.\n` +
          `Every platform must be present — publishing a partial set would ` +
          `leave the wrapper pinning a version that was never published.`,
      );
    }

    const dir = path.join(out, platform.key);
    mkdirSync(path.join(dir, "bin"), { recursive: true });
    const dest = path.join(dir, "bin", binary);
    copyFileSync(source, dest);
    // npm restores the mode from the tarball, so setting it here is what makes
    // the installed binary runnable without a manual chmod.
    if (platform.os !== "win32") chmodSync(dest, 0o755);

    writeJson(path.join(dir, "package.json"), platformManifest(version, platform));
    writeFileSync(path.join(dir, "README.md"), platformReadme(platform, version));
  }

  const wrapperOut = path.join(out, "wrapper");
  mkdirSync(wrapperOut, { recursive: true });
  copySources(path.join(HERE, "wrapper", "bin"), path.join(wrapperOut, "bin"));
  copySources(path.join(HERE, "wrapper", "lib"), path.join(wrapperOut, "lib"));
  writeJson(path.join(wrapperOut, "package.json"), wrapperManifest(version));
  writeFileSync(path.join(wrapperOut, "README.md"), wrapperReadme(version));

  const plan = {
    version,
    distTag: distTagFor(version),
    // Consumed by the publish job so the ordering guarantee lives in tested
    // code rather than in the order of steps in a workflow file.
    order: publishOrder(),
  };
  writeJson(path.join(out, "publish-plan.json"), plan);

  console.log(
    `Built ${PLATFORMS.length + 1} packages for ${version} ` +
      `(dist-tag: ${plan.distTag}) in ${out}`,
  );
  for (const key of plan.order) console.log(`  ${key}`);
}

// Run only when invoked as a CLI, for the same reason as npm/publish.mjs: a
// module that runs its main() on import cannot be unit-tested.
const invokedDirectly =
  process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (invokedDirectly) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
