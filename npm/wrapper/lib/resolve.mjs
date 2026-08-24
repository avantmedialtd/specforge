// Platform resolution for the `@avantmedia/specforge` wrapper.
//
// This module ships inside the published wrapper package, so it must be plain
// JavaScript that runs under a bare `node` with no build step — the repo's
// TypeScript config deliberately scopes `include` to `src`, and nothing here is
// compiled before publication.
//
// The wrapper carries no binary of its own. Each supported target is published
// as a separate `@avantmedia/specforge-<os>-<cpu>` package, declared in the
// wrapper's `optionalDependencies` and filtered by npm's `os`/`cpu` fields, so
// installing the wrapper downloads exactly one of them. Everything below is the
// logic that finds that one package at run time.

import path from "node:path";

/** npm's platform key for a target: `process.platform` + `-` + `process.arch`. */
export function platformKey(platform, arch) {
  return `${platform}-${arch}`;
}

/** The published platform package name for a platform key. */
export function packageNameFor(key) {
  return `@avantmedia/specforge-${key}`;
}

/** The executable's file name, which carries an extension only on Windows. */
export function binaryNameFor(platform) {
  return platform === "win32" ? "specforge-serve.exe" : "specforge-serve";
}

/**
 * The platform keys this wrapper was published with.
 *
 * Read from the wrapper's own `optionalDependencies` rather than a hard-coded
 * list: the generator writes that field and the platform packages from one
 * table, so deriving the keys back out keeps a single source of truth and means
 * a wrapper can never advertise support it did not ship.
 */
export function supportedKeysFrom(optionalDependencies) {
  const prefix = packageNameFor("");
  return Object.keys(optionalDependencies ?? {})
    .filter((name) => name.startsWith(prefix))
    .map((name) => name.slice(prefix.length))
    .sort();
}

/**
 * Locate the platform binary, or return `null` when no platform package is
 * present.
 *
 * `null` covers two distinct situations that are indistinguishable here and
 * identical to the caller: the host is a target we never published, or the host
 * is supported but the package manager resolved the `optionalDependencies` to
 * nothing. The latter is a recurring npm defect rather than a theoretical case,
 * which is why a missing package is a value to handle and not an exception to
 * let escape.
 *
 * `resolvePackageJson` is injected so the failure branches stay testable
 * without publishing or installing anything.
 */
export function locateBinary({ platform, arch, resolvePackageJson }) {
  const pkg = packageNameFor(platformKey(platform, arch));
  let manifestPath;
  try {
    manifestPath = resolvePackageJson(`${pkg}/package.json`);
  } catch {
    return null;
  }
  return path.join(path.dirname(manifestPath), "bin", binaryNameFor(platform));
}

/**
 * The message shown when no binary could be located.
 *
 * It names the detected platform explicitly, because the most common report is
 * "it doesn't work on my machine" from a container whose architecture the user
 * has not thought about, and points at the release downloads so an unsupported
 * platform still has a path forward.
 */
export function unsupportedMessage(platform, arch, supportedKeys) {
  const key = platformKey(platform, arch);
  const known = supportedKeys.includes(key);

  const reason = known
    ? `The platform package ${packageNameFor(key)} is not installed.\n` +
      `Your platform (${key}) is supported, so this is usually a package\n` +
      `manager that resolved optional dependencies to nothing — a known npm\n` +
      `issue. Reinstalling, or removing the lockfile and node_modules and\n` +
      `installing again, normally fixes it.`
    : `No SpecForge server build is published for your platform (${key}).\n` +
      `Published platforms: ${supportedKeys.join(", ")}.`;

  return (
    `specforge-serve: could not find a binary to run.\n\n${reason}\n\n` +
    `Prebuilt archives for every released platform are available at\n` +
    `https://github.com/avantmedialtd/specforge/releases/latest`
  );
}

/**
 * Translate a finished child process into this process's exit code.
 *
 * A child killed by a signal reports `status: null`, and exiting 0 there would
 * report success for a server that was terminated. The shell convention of
 * `128 + signal` keeps `Ctrl-C` distinguishable from a clean shutdown.
 */
export function exitCodeFor({ status, signal }) {
  if (typeof status === "number") return status;
  if (signal) {
    const numbers = { SIGINT: 2, SIGQUIT: 3, SIGKILL: 9, SIGTERM: 15 };
    return 128 + (numbers[signal] ?? 0);
  }
  return 1;
}
