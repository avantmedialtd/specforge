// Maintainer-side packaging logic for the npm distribution channel.
//
// This module is NOT published — it builds the packages that are. It holds the
// pure parts (the platform table, manifest shapes, dist-tag selection, publish
// ordering) so they can be unit-tested; `build-packages.mjs` wraps them in the
// filesystem work.

const SCOPE = "@avantmedia";
const WRAPPER_NAME = `${SCOPE}/specforge`;
const REPOSITORY = "https://github.com/avantmedialtd/specforge";

/**
 * Every target published to npm.
 *
 * `os` and `cpu` are what the package manager filters on, so that installing
 * the wrapper downloads exactly one of these and never a binary the machine
 * cannot execute.
 *
 * The Linux entries deliberately carry NO `libc` field. The Linux binaries are
 * statically linked against musl, which means they also run on glibc
 * distributions — declaring `libc: ["musl"]` would read as obviously correct
 * and would exclude every Debian, Ubuntu, and RHEL user from the package that
 * serves them best. Do not "fix" this by adding the field.
 */
export const PLATFORMS = [
  { key: "darwin-arm64", os: "darwin", cpu: "arm64", label: "macOS (Apple Silicon)" },
  { key: "darwin-x64", os: "darwin", cpu: "x64", label: "macOS (Intel)" },
  { key: "linux-x64", os: "linux", cpu: "x64", label: "Linux (x64, static musl)" },
  { key: "linux-arm64", os: "linux", cpu: "arm64", label: "Linux (arm64, static musl)" },
  { key: "win32-x64", os: "win32", cpu: "x64", label: "Windows (x64)" },
];

export const platformPackageName = (key) => `${SCOPE}/specforge-${key}`;
export const wrapperPackageName = () => WRAPPER_NAME;

/** The executable's file name, which carries an extension only on Windows. */
export const binaryNameFor = (os) =>
  os === "win32" ? "specforge-serve.exe" : "specforge-serve";

/**
 * The dist-tag a version publishes under.
 *
 * A version carrying a prerelease suffix must not become the default install:
 * `npx @avantmedia/specforge` resolves `latest`, so tagging a release candidate
 * `latest` would hand every drive-by user an RC.
 */
export function distTagFor(version) {
  return version.includes("-") ? "next" : "latest";
}

/**
 * The order packages must be published in.
 *
 * Platform packages first, wrapper last. An npm publish cannot be reliably
 * retracted, so ordering is the entire mitigation: if a platform package fails
 * midway the wrapper is simply never published, leaving unreferenced orphans
 * rather than a wrapper whose pinned dependencies do not exist.
 */
export function publishOrder(platforms = PLATFORMS) {
  return [...platforms.map((p) => p.key), "wrapper"];
}

/** Reject anything that is not a plain `x.y.z`, optionally with a suffix. */
export function assertVersion(version) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
    throw new Error(
      `Refusing to build packages for version ${JSON.stringify(version)} — ` +
        `expected x.y.z, optionally with a prerelease suffix.`,
    );
  }
  return version;
}

const common = (version) => ({
  version,
  license: "MIT",
  repository: { type: "git", url: `git+${REPOSITORY}.git` },
  homepage: `${REPOSITORY}#readme`,
  bugs: { url: `${REPOSITORY}/issues` },
});

/**
 * The manifest for one platform package: metadata plus a single executable.
 * No `main`, no `exports` — nothing here is importable, and leaving `exports`
 * unset is also what keeps the wrapper's `require.resolve` of this package's
 * `package.json` working.
 */
export function platformManifest(version, platform) {
  return {
    name: platformPackageName(platform.key),
    ...common(version),
    description: `SpecForge headless web server binary for ${platform.label}.`,
    os: [platform.os],
    cpu: [platform.cpu],
    files: ["bin", "README.md"],
  };
}

/** The manifest for the wrapper: a bin shim and exact pins, no binary. */
export function wrapperManifest(version, platforms = PLATFORMS) {
  const optionalDependencies = {};
  for (const platform of platforms) {
    // Exact pins, no range operator: the wrapper and its platform packages are
    // published together from one tag and are only ever valid as a set.
    optionalDependencies[platformPackageName(platform.key)] = version;
  }

  return {
    name: WRAPPER_NAME,
    ...common(version),
    description:
      "Run the SpecForge headless web server — browse OpenSpec workspaces in a browser.",
    keywords: ["specforge", "openspec", "spec-driven", "cli", "server"],
    bin: { "specforge-serve": "bin/specforge-serve.mjs" },
    files: ["bin", "lib", "README.md"],
    engines: { node: ">=18" },
    optionalDependencies,
  };
}
