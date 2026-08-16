import { describe, expect, test } from "bun:test"
import { clipboardStrategy } from "./clipboard"

// The strategy choice is the load-bearing decision in `clipboard.ts`: pick
// `async` where the origin has no Clipboard API and the copy silently does
// nothing; pick `selection` where it does have one and the deprecated
// synchronous path is used for no reason. Both failures are invisible in the
// host that gets developed against (loopback, which has the API), so they are
// pinned here.

describe("clipboardStrategy", () => {
    test("chooses async when the Clipboard API is exposed", () => {
        expect(clipboardStrategy({ clipboard: { writeText: () => {} } })).toBe("async")
    })

    // The non-loopback `--bind` case: plain HTTP on a non-localhost origin is
    // not a secure context, so the whole `navigator.clipboard` object is
    // absent. Verified in a real browser against such a bind.
    test("chooses selection when navigator.clipboard is undefined", () => {
        expect(clipboardStrategy({})).toBe("selection")
    })

    // Defensive: some environments expose a partial `clipboard` object (e.g.
    // read-only permissions) where `writeText` is missing. Probing the object
    // rather than the method would pick `async` and then throw on use.
    test("chooses selection when clipboard exists but writeText does not", () => {
        expect(clipboardStrategy({ clipboard: {} })).toBe("selection")
    })

    test("chooses selection when writeText is present but not callable", () => {
        expect(
            clipboardStrategy({ clipboard: { writeText: "nope" } }),
        ).toBe("selection")
    })

    test("chooses selection when there is no navigator at all", () => {
        expect(clipboardStrategy(undefined)).toBe("selection")
    })
})
