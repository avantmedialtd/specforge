import { describe, expect, test } from "bun:test";
import { isPublishConflict } from "./publish.mjs";

describe("isPublishConflict", () => {
  test("recognises the registry's conflict error code", () => {
    expect(
      isPublishConflict(
        "npm error code EPUBLISHCONFLICT\nnpm error 403 Forbidden - PUT https://registry.npmjs.org/@avantmedia%2fspecforge",
      ),
    ).toBe(true);
  });

  test("recognises the prose form the registry also uses", () => {
    expect(
      isPublishConflict(
        "npm error 403 You cannot publish over the previously published versions: 0.19.0.",
      ),
    ).toBe(true);
  });

  test("is case-insensitive", () => {
    expect(isPublishConflict("Cannot Publish Over the previous version")).toBe(
      true,
    );
  });

  // This is the direction that matters. Treating any of these as "already
  // published" would silently skip a package that genuinely failed, and the
  // wrapper would then be published pinning a version that does not exist —
  // exactly the state the publish ordering exists to prevent.
  test("does not treat an authentication failure as a conflict", () => {
    expect(
      isPublishConflict(
        "npm error code ENEEDAUTH\nnpm error need auth This command requires you to be logged in to https://registry.npmjs.org/",
      ),
    ).toBe(false);
  });

  test("does not treat a payment/access failure as a conflict", () => {
    expect(
      isPublishConflict(
        "npm error code E402\nnpm error 402 Payment Required - PUT https://registry.npmjs.org/@avantmedia%2fspecforge",
      ),
    ).toBe(false);
  });

  test("does not treat a network failure as a conflict", () => {
    expect(
      isPublishConflict("npm error code ECONNRESET\nnpm error network"),
    ).toBe(false);
  });

  test("does not treat a forbidden-scope failure as a conflict", () => {
    expect(
      isPublishConflict(
        "npm error 403 Forbidden - You do not have permission to publish \"@avantmedia/specforge\".",
      ),
    ).toBe(false);
  });

  test("empty and absent stderr are not conflicts", () => {
    expect(isPublishConflict("")).toBe(false);
    expect(isPublishConflict(undefined)).toBe(false);
    expect(isPublishConflict(null)).toBe(false);
  });
});
