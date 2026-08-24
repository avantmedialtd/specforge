import { describe, expect, test } from "bun:test";
import {
  binaryNameFor,
  exitCodeFor,
  locateBinary,
  packageNameFor,
  platformKey,
  supportedKeysFrom,
  unsupportedMessage,
} from "./resolve.mjs";

const PUBLISHED = {
  "@avantmedia/specforge-darwin-arm64": "0.19.0",
  "@avantmedia/specforge-darwin-x64": "0.19.0",
  "@avantmedia/specforge-linux-arm64": "0.19.0",
  "@avantmedia/specforge-linux-x64": "0.19.0",
  "@avantmedia/specforge-win32-x64": "0.19.0",
};

describe("platformKey", () => {
  test("joins platform and arch the way npm names targets", () => {
    expect(platformKey("darwin", "arm64")).toBe("darwin-arm64");
    expect(platformKey("linux", "x64")).toBe("linux-x64");
    expect(platformKey("win32", "x64")).toBe("win32-x64");
  });
});

describe("packageNameFor", () => {
  test("scopes the package, never publishing an unscoped name", () => {
    expect(packageNameFor("linux-arm64")).toBe(
      "@avantmedia/specforge-linux-arm64",
    );
  });
});

describe("binaryNameFor", () => {
  test("only Windows carries an extension", () => {
    expect(binaryNameFor("win32")).toBe("specforge-serve.exe");
    expect(binaryNameFor("darwin")).toBe("specforge-serve");
    expect(binaryNameFor("linux")).toBe("specforge-serve");
  });
});

describe("supportedKeysFrom", () => {
  test("derives every published platform key, sorted", () => {
    expect(supportedKeysFrom(PUBLISHED)).toEqual([
      "darwin-arm64",
      "darwin-x64",
      "linux-arm64",
      "linux-x64",
      "win32-x64",
    ]);
  });

  test("ignores dependencies that are not platform packages", () => {
    expect(
      supportedKeysFrom({
        "@avantmedia/specforge-linux-x64": "0.19.0",
        "some-unrelated-package": "1.0.0",
      }),
    ).toEqual(["linux-x64"]);
  });

  test("a wrapper with no optional dependencies advertises nothing", () => {
    expect(supportedKeysFrom(undefined)).toEqual([]);
    expect(supportedKeysFrom({})).toEqual([]);
  });
});

describe("locateBinary", () => {
  test("resolves the binary beside the platform package's manifest", () => {
    const found = locateBinary({
      platform: "linux",
      arch: "x64",
      resolvePackageJson: (specifier: string) => {
        expect(specifier).toBe("@avantmedia/specforge-linux-x64/package.json");
        return "/app/node_modules/@avantmedia/specforge-linux-x64/package.json";
      },
    });
    expect(found).toBe(
      "/app/node_modules/@avantmedia/specforge-linux-x64/bin/specforge-serve",
    );
  });

  test("uses the .exe name on Windows", () => {
    const found = locateBinary({
      platform: "win32",
      arch: "x64",
      resolvePackageJson: () =>
        "/app/node_modules/@avantmedia/specforge-win32-x64/package.json",
    });
    expect(found?.endsWith("specforge-serve.exe")).toBe(true);
  });

  // The npm defect where optionalDependencies resolve to nothing surfaces here
  // as a throwing resolver. It has to become a value the caller can report, not
  // an exception that escapes as a module-resolution stack trace.
  test("returns null instead of throwing when no package is installed", () => {
    const found = locateBinary({
      platform: "linux",
      arch: "arm64",
      resolvePackageJson: () => {
        throw new Error("Cannot find module");
      },
    });
    expect(found).toBeNull();
  });
});

describe("unsupportedMessage", () => {
  test("names the detected platform", () => {
    const message = unsupportedMessage("linux", "riscv64", ["linux-x64"]);
    expect(message).toContain("linux-riscv64");
  });

  test("an unpublished platform lists what was published", () => {
    const message = unsupportedMessage("linux", "riscv64", [
      "linux-arm64",
      "linux-x64",
    ]);
    expect(message).toContain("No SpecForge server build is published");
    expect(message).toContain("linux-arm64, linux-x64");
  });

  test("a supported platform is diagnosed as a missing install, not a gap", () => {
    const message = unsupportedMessage(
      "darwin",
      "arm64",
      supportedKeysFrom(PUBLISHED),
    );
    expect(message).toContain("@avantmedia/specforge-darwin-arm64");
    expect(message).toContain("supported");
    expect(message).not.toContain("No SpecForge server build is published");
  });

  test("always offers the release downloads as a way forward", () => {
    const message = unsupportedMessage("sunos", "x64", ["linux-x64"]);
    expect(message).toContain("releases/latest");
  });
});

describe("exitCodeFor", () => {
  test("a normal exit propagates its status", () => {
    expect(exitCodeFor({ status: 0, signal: null })).toBe(0);
    // A refused unsafe bind exits non-zero; the shim must not flatten it to 0.
    expect(exitCodeFor({ status: 1, signal: null })).toBe(1);
    expect(exitCodeFor({ status: 42, signal: null })).toBe(42);
  });

  test("a signalled exit reports 128 + signal rather than success", () => {
    expect(exitCodeFor({ status: null, signal: "SIGINT" })).toBe(130);
    expect(exitCodeFor({ status: null, signal: "SIGTERM" })).toBe(143);
  });

  test("an unknown outcome fails rather than reporting success", () => {
    expect(exitCodeFor({ status: null, signal: null })).toBe(1);
  });
});
