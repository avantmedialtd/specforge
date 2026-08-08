import { describe, expect, test } from "bun:test"
import { partitionFavorites } from "./favorites"

const keyOf = (s: string) => `k:${s}`
const favs = (...names: string[]) => new Set(names.map(keyOf))

describe("partitionFavorites", () => {
    test("favorites float to the front, name order preserved in each half", () => {
        // The spec's worked example: favorites delta+bravo over
        // charlie+alpha renders bravo, delta, alpha, charlie.
        const items = ["alpha", "bravo", "charlie", "delta"]
        expect(partitionFavorites(items, favs("delta", "bravo"), keyOf)).toEqual(
            ["bravo", "delta", "alpha", "charlie"],
        )
    })

    test("single favorite floats over the rest", () => {
        const items = ["alpha", "mid", "zulu"]
        expect(partitionFavorites(items, favs("zulu"), keyOf)).toEqual([
            "zulu",
            "alpha",
            "mid",
        ])
    })

    test("no favorites returns the input array identity", () => {
        const items = ["alpha", "mid"]
        expect(partitionFavorites(items, new Set(), keyOf)).toBe(items)
    })

    test("favorites matching no item return the input array identity", () => {
        const items = ["alpha", "mid"]
        expect(partitionFavorites(items, favs("other"), keyOf)).toBe(items)
    })

    test("all items favorited keeps the backend order", () => {
        const items = ["alpha", "mid", "zulu"]
        expect(
            partitionFavorites(items, favs("zulu", "alpha", "mid"), keyOf),
        ).toEqual(["alpha", "mid", "zulu"])
    })

    test("empty input stays empty", () => {
        expect(partitionFavorites([], favs("x"), keyOf)).toEqual([])
    })
})
