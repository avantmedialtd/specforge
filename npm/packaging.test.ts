import { describe, expect, test } from "bun:test";
import {
  PLATFORMS,
  assertVersion,
  binaryNameFor,
  distTagFor,
  platformManifest,
  platformPackageName,
  publishOrder,
  wrapperManifest,
  wrapperPackageName,
} from "./packaging.mjs";

describe("PLATFORMS", () => {
  test("covers exactly the five published targets", () => {
    expect(PLATFORMS.map((p: { key: string }) => p.key).sort()).toEqual([
      "darwin-arm64",
      "darwin-x64",
      "linux-arm64",
      "linux-x64",
      "win32-x64",
    ]);
  });

  test("every platform declares a single os and cpu to filter on", () => {
    for (const platform of PLATFORMS) {
      const manifest = platformManifest("0.19.0", platform);
      expect(manifest.os).toEqual([platform.os]);
      expect(manifest.cpu).toEqual([platform.cpu]);
    }
  });
});

describe("platform manifests", () => {
  // A static musl binary runs on glibc systems too. Declaring libc: ["musl"]
  // looks correct and would exclude most Linux users from the package that
  // serves them best, so its absence is asserted rather than assumed.
  test("Linux packages declare no libc constraint", () => {
    const linux = PLATFORMS.filter((p: { os: string }) => p.os === "linux");
    expect(linux.length).toBe(2);
    for (const platform of linux) {
      expect(platformManifest("0.19.0", platform)).not.toHaveProperty("libc");
    }
  });

  test("no platform package declares a libc constraint at all", () => {
    for (const platform of PLATFORMS) {
      expect(platformManifest("0.19.0", platform)).not.toHaveProperty("libc");
    }
  });

  test("names are scoped", () => {
    for (const platform of PLATFORMS) {
      expect(platformManifest("0.19.0", platform).name).toStartWith(
        "@avantmedia/",
      );
    }
  });

  // Leaving `exports` unset is what keeps the wrapper's require.resolve of this
  // package's package.json working.
  test("platform packages define no exports map", () => {
    for (const platform of PLATFORMS) {
      expect(platformManifest("0.19.0", platform)).not.toHaveProperty("exports");
    }
  });

  test("ships only the binary directory and its readme", () => {
    expect(platformManifest("0.19.0", PLATFORMS[0]).files).toEqual([
      "bin",
      "README.md",
    ]);
  });
});

describe("wrapperManifest", () => {
  test("pins every platform package to the exact version", () => {
    const manifest = wrapperManifest("0.19.0");
    const deps = manifest.optionalDependencies as Record<string, string>;
    expect(Object.keys(deps).sort()).toEqual(
      PLATFORMS.map((p: { key: string }) => platformPackageName(p.key)).sort(),
    );
    for (const range of Object.values(deps)) {
      expect(range).toBe("0.19.0");
    }
  });

  test("carries the product name inside the scope", () => {
    expect(wrapperManifest("0.19.0").name).toBe("@avantmedia/specforge");
    expect(wrapperPackageName()).toBe("@avantmedia/specforge");
  });

  test("exposes the specforge-serve bin", () => {
    expect(wrapperManifest("0.19.0").bin).toEqual({
      "specforge-serve": "bin/specforge-serve.mjs",
    });
  });

  test("declares platform packages as optional, never required", () => {
    const manifest = wrapperManifest("0.19.0");
    expect(manifest).not.toHaveProperty("dependencies");
    expect(manifest.optionalDependencies).toBeDefined();
  });
});

describe("distTagFor", () => {
  test("stable versions publish to latest", () => {
    expect(distTagFor("0.19.0")).toBe("latest");
    expect(distTagFor("1.0.0")).toBe("latest");
  });

  // Publishing an RC to `latest` would hand every `npx` user a prerelease.
  test("prerelease versions publish to next", () => {
    expect(distTagFor("0.20.0-rc.1")).toBe("next");
    expect(distTagFor("1.0.0-beta.2")).toBe("next");
  });
});

describe("publishOrder", () => {
  test("publishes the wrapper last, after every platform package", () => {
    const order = publishOrder();
    expect(order[order.length - 1]).toBe("wrapper");
    expect(order.length).toBe(PLATFORMS.length + 1);
  });

  test("includes every platform exactly once", () => {
    const order = publishOrder().filter((k: string) => k !== "wrapper");
    expect(order.sort()).toEqual(PLATFORMS.map((p: { key: string }) => p.key).sort());
  });
});

describe("assertVersion", () => {
  test("accepts plain and prerelease semver", () => {
    expect(assertVersion("0.19.0")).toBe("0.19.0");
    expect(assertVersion("1.2.3-rc.1")).toBe("1.2.3-rc.1");
  });

  test("rejects anything that would publish a wrong version", () => {
    expect(() => assertVersion("v0.19.0")).toThrow();
    expect(() => assertVersion("0.19")).toThrow();
    expect(() => assertVersion("")).toThrow();
    expect(() => assertVersion(undefined)).toThrow();
  });
});

describe("binaryNameFor", () => {
  test("only Windows carries an extension", () => {
    expect(binaryNameFor("win32")).toBe("specforge-serve.exe");
    expect(binaryNameFor("linux")).toBe("specforge-serve");
    expect(binaryNameFor("darwin")).toBe("specforge-serve");
  });
});
