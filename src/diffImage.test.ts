import { describe, expect, test } from "bun:test"
import {
    decodeBase64,
    decodeSide,
    decodeVersions,
    differenceAvailable,
    differenceName,
    formatBytes,
    holdsNoImage,
    imageDetails,
    NearViewQueue,
    refusalText,
    sizeDelta,
    UNDRAWABLE_TEXT,
    versionName,
    type DecodedImage,
    type DecodedSide,
} from "./diffImage"

const KIB = 1024
const MIB = 1024 * 1024

function image(width: number, height: number, size = 0): DecodedSide {
    return {
        kind: "image",
        image: { mime: "image/png", width, height, bytes: new Uint8Array(size) },
    }
}

function decoded(width: number, height: number, size: number): DecodedImage {
    return { mime: "image/png", width, height, bytes: new Uint8Array(size) }
}

describe("decoding", () => {
    test("base64 decodes to its bytes", () => {
        expect([...decodeBase64("iVBORw0KGgo=")]).toEqual([
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
        ])
        expect(decodeBase64("").length).toBe(0)
    })

    test("each side decodes as a read answered it", () => {
        expect(
            decodeSide({ kind: "image", mime: "image/gif", width: 3, height: 2, data: "R0lG" }),
        ).toEqual({
            kind: "image",
            image: { mime: "image/gif", width: 3, height: 2, bytes: new Uint8Array([71, 73, 70]) },
        })
        expect(decodeSide({ kind: "absent" })).toEqual({ kind: "absent" })
        expect(decodeSide({ kind: "refused", reason: "lfs" })).toEqual({
            kind: "refused",
            reason: "lfs",
        })
        expect(
            decodeVersions({ old: { kind: "absent" }, new: { kind: "refused", reason: "tooLarge" } }),
        ).toEqual({ old: { kind: "absent" }, new: { kind: "refused", reason: "tooLarge" } })
    })

    test("a read with every side absent or not an image holds no image", () => {
        const notImage: DecodedSide = { kind: "refused", reason: "notImage" }
        const absent: DecodedSide = { kind: "absent" }
        expect(holdsNoImage({ old: notImage, new: notImage })).toBe(true)
        expect(holdsNoImage({ old: absent, new: notImage })).toBe(true)
        expect(holdsNoImage({ old: notImage, new: image(3, 2) })).toBe(false)
        expect(holdsNoImage({ old: absent, new: { kind: "refused", reason: "tooLarge" } })).toBe(
            false,
        )
        expect(holdsNoImage({ old: { kind: "refused", reason: "lfs" }, new: absent })).toBe(false)
    })
})

describe("sizes", () => {
    test("a size is bytes below 1 KB, and one decimal below 10 KB or MB", () => {
        expect(formatBytes(0)).toBe("0 B")
        expect(formatBytes(812)).toBe("812 B")
        expect(formatBytes(KIB - 1)).toBe("1023 B")
        expect(formatBytes(KIB)).toBe("1.0 KB")
        expect(formatBytes(9.94 * KIB)).toBe("9.9 KB")
        expect(formatBytes(9.96 * KIB)).toBe("10 KB")
        expect(formatBytes(345 * KIB)).toBe("345 KB")
        expect(formatBytes(MIB - 1)).toBe("1024 KB")
        expect(formatBytes(MIB)).toBe("1.0 MB")
        expect(formatBytes(1.4 * MIB)).toBe("1.4 MB")
        expect(formatBytes(12.6 * MIB)).toBe("13 MB")
    })

    test("a change in size is bytes and a whole percentage of the old size", () => {
        expect(sizeDelta(345 * KIB, 298 * KIB)).toBe("−47 KB (−14%)")
        expect(sizeDelta(100, 150)).toBe("+50 B (+50%)")
        expect(sizeDelta(100, 100)).toBe("0 B (0%)")
        expect(sizeDelta(1000, 999)).toBe("−1 B (0%)")
        expect(sizeDelta(0, 812)).toBe("+812 B")
    })

    test("a caption gives dimensions, size and, for the new side, the change", () => {
        const before = decoded(512, 512, 345 * KIB)
        const after = decoded(512, 512, 298 * KIB)
        expect(imageDetails(before)).toBe("512 × 512 · 345 KB")
        expect(imageDetails(after, before)).toBe("512 × 512 · 298 KB · −47 KB (−14%)")
    })
})

describe("the Difference mode", () => {
    test("is offered only for two images of equal dimensions", () => {
        expect(differenceAvailable({ old: image(512, 512), new: image(512, 512) })).toBe(true)
        expect(differenceAvailable({ old: image(512, 512), new: image(1024, 1024) })).toBe(false)
        expect(differenceAvailable({ old: image(512, 256), new: image(512, 512) })).toBe(false)
        expect(differenceAvailable({ old: image(256, 512), new: image(512, 512) })).toBe(false)
        expect(differenceAvailable({ old: { kind: "absent" }, new: image(16, 16) })).toBe(false)
        expect(
            differenceAvailable({ old: image(16, 16), new: { kind: "refused", reason: "lfs" } }),
        ).toBe(false)
    })
})

describe("wording", () => {
    test("each refusal reads its own words", () => {
        expect(refusalText("lfs")).toBe("Stored in Git LFS")
        expect(refusalText("tooLarge")).toBe("Too large to preview")
        expect(refusalText("notImage")).toBe("Not an image this view can show")
        expect(refusalText("tooManyPixels")).toBe("Too many pixels to preview")
        expect(UNDRAWABLE_TEXT).toBe("Can't be shown here")
    })

    test("versions are named by path and side", () => {
        expect(versionName("icons/app.png", "abc1234")).toBe("icons/app.png at abc1234")
        expect(differenceName("icons/app.png", "abc1234", "def5678")).toBe(
            "icons/app.png: difference between abc1234 and def5678",
        )
    })
})

/// A queue whose reads the test ends one at a time, in any order, each
/// succeeding or failing.
function scripted(limit?: number) {
    const started: string[] = []
    const ends = new Map<string, (failed: boolean) => void>()
    const queue = new NearViewQueue(
        (key) =>
            new Promise<void>((resolve, reject) => {
                started.push(key)
                ends.set(key, (failed) => (failed ? reject(new Error(key)) : resolve()))
            }),
        limit,
    )
    const end = async (key: string, failed = false) => {
        ends.get(key)?.(failed)
        // Let the read's continuation run.
        for (let i = 0; i < 5; i += 1) await Promise.resolve()
    }
    return { queue, started, end }
}

describe("the near-view queue", () => {
    test("reads at most two at once, each next one starting as a read ends", async () => {
        const { queue, started, end } = scripted()
        for (const key of ["a", "b", "c", "d", "e"]) queue.near(key)
        expect(started).toEqual(["a", "b"])
        expect(queue.snapshot()).toEqual({ waiting: ["c", "d", "e"], reading: ["a", "b"] })
        await end("b")
        expect(started).toEqual(["a", "b", "c"])
        await end("a")
        await end("c")
        expect(started).toEqual(["a", "b", "c", "d", "e"])
    })

    test("a section that leaves the margin before its read starts is dropped", async () => {
        const { queue, started, end } = scripted()
        for (const key of ["a", "b", "c", "d"]) queue.near(key)
        queue.left("c")
        // Leaving once its read started stops nothing.
        queue.left("a")
        await end("a")
        await end("b")
        expect(started).toEqual(["a", "b", "d"])
        expect(queue.snapshot()).toEqual({ waiting: [], reading: ["d"] })
    })

    test("a file read once, waiting or being read never joins again", async () => {
        const { queue, started, end } = scripted()
        queue.near("a")
        queue.near("a")
        queue.near("b")
        queue.near("c")
        queue.near("c")
        await end("a")
        queue.near("a")
        expect(started).toEqual(["a", "b", "c"])
        expect(queue.snapshot().waiting).toEqual([])
    })

    test("a read that fails still frees its place, and trying again reads it next", async () => {
        const { queue, started, end } = scripted(1)
        queue.near("a")
        queue.near("b")
        await end("a", true)
        expect(started).toEqual(["a", "b"])
        // Failed, it is read once, and joins no more by coming near.
        queue.near("a")
        queue.near("c")
        expect(queue.snapshot().waiting).toEqual(["c"])
        queue.retry("a")
        expect(queue.snapshot().waiting).toEqual(["a", "c"])
        await end("b")
        await end("a")
        expect(started).toEqual(["a", "b", "a", "c"])
    })

    test("trying again with nothing in flight reads at once", () => {
        const { queue, started } = scripted()
        queue.retry("a")
        expect(started).toEqual(["a"])
    })
})
