import { describe, expect, test } from "bun:test"
import {
    decodeBase64,
    decodeSide,
    decodeVersions,
    comparisonAvailable,
    comparisonName,
    IMAGE_MODE_OPTIONS,
    formatBytes,
    holdsNoImage,
    imageDetails,
    NearViewQueue,
    clampReveal,
    revealAt,
    revealKey,
    pixelDiffSummary,
    refusalText,
    sizeDelta,
    UNDRAWABLE_TEXT,
    versionName,
    type DecodedImage,
    type DecodedSide,
} from "./diffImage"
import { exifJpeg } from "./testing/exifJpeg"
import { canHighlight, comparisonSize } from "./imageDiffPixels"

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

    test("EXIF orientation determines JPEG dimensions before frames or pixels are allocated", () => {
        for (const little of [false, true]) {
            for (const orientation of [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]) {
                const bytes = exifJpeg(orientation, little)
                const side = decodeSide({
                    kind: "image", mime: "image/jpeg", width: 200, height: 100,
                    data: btoa(String.fromCharCode(...bytes)),
                })
                if (side.kind !== "image") { throw new Error("accepted JPEG was refused") }
                const swapped = orientation >= 5 && orientation <= 8
                expect([side.image.width, side.image.height]).toEqual(swapped ? [100, 200] : [200, 100])
                expect(side.image.bytes).toEqual(bytes)
            }
        }
    })

    test("orientation-aware union extents enforce the pixel ceiling before decoding", () => {
        const pair = decodeVersions({
            old: { kind: "image", mime: "image/jpeg", width: 3000, height: 1000,
                data: btoa(String.fromCharCode(...exifJpeg(6))) },
            new: { kind: "image", mime: "image/png", width: 3000, height: 1000, data: "" },
        })
        if (pair.old.kind !== "image" || pair.new.kind !== "image") { throw new Error("missing version") }
        expect(comparisonSize(pair.old.image, pair.new.image)).toEqual({ width: 3000, height: 3000 })
        expect(canHighlight(pair.old.image, pair.new.image)).toBe(false)
    })

    test("JPEG metadata walking skips other segments and stops before compressed pixels", () => {
        const exif = exifJpeg(6)
        const withOtherSegments = Uint8Array.from([
            ...exif.slice(0, 2),
            0xff, 0xe0, 0, 4, 0, 0, // APP0
            0xff, 0xe1, 0, 8, 1, 2, 3, 4, 5, 6, // Not EXIF (e.g. XMP)
            0xff, 0xff, 0x01, // Fill byte and a standalone marker
            ...exif.slice(2),
        ])
        const inScan = Uint8Array.from([0xff, 0xd8, 0xff, 0xda, ...exif.slice(2)])
        for (const [bytes, width, height] of [
            [withOtherSegments, 100, 200], [inScan, 200, 100],
        ] as const) {
            expect(decodeSide({ kind: "image", mime: "image/jpeg", width: 200, height: 100,
                data: btoa(String.fromCharCode(...bytes)) }))
                .toMatchObject({ kind: "image", image: { width, height } })
        }
    })

    test("malformed or truncated EXIF metadata safely keeps the service dimensions", () => {
        const corruptions = [
            (bytes: Uint8Array) => { bytes[4] = 0xff },
            (bytes: Uint8Array) => { bytes[12] = 0 },
            (bytes: Uint8Array) => { bytes[15] = 0 },
            (bytes: Uint8Array) => { bytes[16] = 0xff },
            (bytes: Uint8Array) => { bytes[21] = 2 },
            (bytes: Uint8Array) => { bytes[25] = 4 },
            (bytes: Uint8Array) => { bytes[29] = 2 },
        ]
        const cases = corruptions.map(corrupt => {
            const bytes = exifJpeg(6)
            corrupt(bytes)
            return bytes
        })
        const complete = exifJpeg(6)
        for (let length = 0; length < 38; length++) { cases.push(complete.slice(0, length)) }
        for (const bytes of cases) {
            const side = decodeSide({
                kind: "image", mime: "image/jpeg", width: 200, height: 100,
                data: btoa(String.fromCharCode(...bytes)),
            })
            expect(side).toMatchObject({ kind: "image", image: { width: 200, height: 100 } })
        }
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

describe("image review modes", () => {
    test("all six modes work with resized images but need two accepted sides", () => {
        expect(IMAGE_MODE_OPTIONS.map(option => option.label)).toEqual([
            "Diff", "Before", "After", "Side by side", "Slider", "Blink",
        ])
        expect(comparisonAvailable({ old: image(512, 512), new: image(1024, 256) })).toBe(true)
        expect(comparisonAvailable({ old: image(1, 1), new: image(1, 1) })).toBe(true)
        for (const missing of [
            { kind: "absent" },
            { kind: "refused", reason: "tooLarge" },
        ] as const) {
            expect(comparisonAvailable({ old: missing, new: image(1, 1) })).toBe(false)
            expect(comparisonAvailable({ old: image(1, 1), new: missing })).toBe(false)
        }
        expect(comparisonName("slider", "screen.png", "base", "head")).toBe(
            "screen.png: Slider between base and head",
        )
    })

    test("a reveal is clamped and usable by pointer and keyboard", () => {
        expect(clampReveal(-5)).toBe(0)
        expect(clampReveal(105)).toBe(100)
        expect(clampReveal(NaN)).toBe(50)
        expect(revealAt(150, 50, 200)).toBe(50)
        expect(revealAt(-10, 50, 200)).toBe(0)
        expect(revealAt(300, 50, 200)).toBe(100)
        expect(revealAt(1, 1, 0)).toBe(50)
        for (const key of ["ArrowRight", "ArrowUp"]) { expect(revealKey(key, 50)).toBe(51) }
        for (const key of ["ArrowLeft", "ArrowDown"]) { expect(revealKey(key, 50)).toBe(49) }
        expect(revealKey("Home", 50)).toBe(0)
        expect(revealKey("End", 50)).toBe(100)
        expect(revealKey("PageUp", 95)).toBe(100)
        expect(revealKey("PageDown", 5)).toBe(0)
        expect(revealKey("Escape", 50)).toBeNull()
    })

    test("pixel summaries distinguish identical, single-pixel and tiny changes", () => {
        expect(pixelDiffSummary(0, 400)).toBe("No pixels changed (0%)")
        expect(pixelDiffSummary(1, 100)).toBe("1 pixel changed (1%)")
        expect(pixelDiffSummary(2304, 40000)).toBe("2,304 pixels changed (5.76%)")
        expect(pixelDiffSummary(1, 4000000)).toBe("1 pixel changed (<0.01%)")
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
        expect(comparisonName("diff", "icons/app.png", "abc1234", "def5678")).toBe(
            "icons/app.png: Diff between abc1234 and def5678",
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
        for (let i = 0; i < 5; i += 1) { await Promise.resolve() }
    }
    return { queue, started, end }
}

describe("the near-view queue", () => {
    test("reads at most two at once, each next one starting as a read ends", async () => {
        const { queue, started, end } = scripted()
        for (const key of ["a", "b", "c", "d", "e"]) { queue.near(key) }
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
        for (const key of ["a", "b", "c", "d"]) { queue.near(key) }
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
