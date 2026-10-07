import { describe, expect, test } from "bun:test"
import { createHash } from "node:crypto"
import { sha256Hex } from "./sha256"

describe("sha256Hex", () => {
    // FIPS 180-2's examples, and the empty string.
    test("matches the standard vectors", () => {
        expect(sha256Hex("")).toBe("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        expect(sha256Hex("abc")).toBe(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        expect(sha256Hex("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")).toBe(
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        )
    })

    test("hashes a path's UTF-8 bytes, as GitHub's file anchors do", () => {
        expect(sha256Hex("src/huge.json")).toBe(
            "dd89c4cf549b7418f9dde6cfa5beea228450b2df2f433c9d98d2c949c9c3fc2e",
        )
        for (const text of ["apps/uk/+Page.tsx", "café/naïve.md", "日本語.txt", "x".repeat(55)]) {
            expect(sha256Hex(text)).toBe(createHash("sha256").update(text, "utf8").digest("hex"))
        }
    })

    // Every length across the padding boundaries (55, 56 and 64 bytes) and
    // into a third block.
    test("agrees with node's digest at every length up to 200", () => {
        for (let length = 0; length <= 200; length++) {
            const text = "ab".repeat(length).slice(0, length)
            expect(sha256Hex(text)).toBe(createHash("sha256").update(text, "utf8").digest("hex"))
        }
    })
})
