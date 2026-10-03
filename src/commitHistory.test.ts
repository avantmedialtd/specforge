import { describe, expect, test } from "bun:test"
import {
    COMMIT_HISTORY_STORAGE_KEY,
    railHasOccupant,
    readMirroredCommitHistory,
    writeMirroredCommitHistory,
    type CommitHistoryStore,
    type PanelPlacement,
} from "./commitHistory"

function fakeStore(initial?: string): CommitHistoryStore & { data: Map<string, string> } {
    const data = new Map<string, string>()
    if (initial !== undefined) data.set(COMMIT_HISTORY_STORAGE_KEY, initial)
    return {
        data,
        getItem: (key) => data.get(key) ?? null,
        setItem: (key, value) => {
            data.set(key, value)
        },
    }
}

const throwingStore: CommitHistoryStore = {
    getItem: () => {
        throw new Error("blocked")
    },
    setItem: () => {
        throw new Error("blocked")
    },
}

describe("the first-paint mirror", () => {
    test("round-trips both values under the documented key", () => {
        const store = fakeStore()
        writeMirroredCommitHistory(false, store)
        expect(store.data.get(COMMIT_HISTORY_STORAGE_KEY)).toBe("false")
        expect(readMirroredCommitHistory(store)).toBe(false)

        writeMirroredCommitHistory(true, store)
        expect(store.data.get(COMMIT_HISTORY_STORAGE_KEY)).toBe("true")
        expect(readMirroredCommitHistory(store)).toBe(true)
    })

    test("the key sits beside the rail's own view state", () => {
        expect(COMMIT_HISTORY_STORAGE_KEY).toBe("specforge.commitHistoryEnabled")
    })

    test("an empty store reads as on — a first run keeps the graph", () => {
        expect(readMirroredCommitHistory(fakeStore())).toBe(true)
    })

    test("only the exact value this module writes reads as off", () => {
        for (const value of ["true", "", "0", "no", "FALSE", " false"]) {
            expect(readMirroredCommitHistory(fakeStore(value))).toBe(true)
        }
        expect(readMirroredCommitHistory(fakeStore("false"))).toBe(false)
    })

    test("a store that throws reads as on and swallows writes", () => {
        expect(readMirroredCommitHistory(throwingStore)).toBe(true)
        expect(() => writeMirroredCommitHistory(false, throwingStore)).not.toThrow()
    })

    test("no store at all reads as on and ignores writes", () => {
        expect(readMirroredCommitHistory(null)).toBe(true)
        expect(() => writeMirroredCommitHistory(false, null)).not.toThrow()
    })
})

describe("railHasOccupant", () => {
    const panel = (position: PanelPlacement["position"], present: boolean): PanelPlacement => ({
        position,
        present,
    })

    test("the commit graph alone occupies the rail", () => {
        expect(railHasOccupant(true, [])).toBe(true)
    })

    test("history off with nothing else leaves no rail", () => {
        expect(railHasOccupant(false, [])).toBe(false)
    })

    test("a present panel in either rail slot occupies it", () => {
        expect(railHasOccupant(false, [panel("right-top", true)])).toBe(true)
        expect(railHasOccupant(false, [panel("right-bottom", true)])).toBe(true)
    })

    test("a panel parked in a rail slot with its feature off does not", () => {
        expect(railHasOccupant(false, [panel("right-top", false), panel("right-bottom", false)])).toBe(
            false,
        )
    })

    test("a present panel in the sidebar does not", () => {
        expect(railHasOccupant(false, [panel("left-top", true), panel("left-bottom", true)])).toBe(
            false,
        )
    })

    test("a panel whose position is not yet read does not", () => {
        expect(railHasOccupant(false, [panel(null, true)])).toBe(false)
    })

    test("one present rail panel is enough, whatever the other does", () => {
        expect(railHasOccupant(false, [panel("left-bottom", true), panel("right-bottom", true)])).toBe(
            true,
        )
        expect(railHasOccupant(false, [panel("right-top", false), panel("right-bottom", true)])).toBe(
            true,
        )
    })

    test("with history on the panels do not matter", () => {
        expect(railHasOccupant(true, [panel("right-top", false)])).toBe(true)
        expect(railHasOccupant(true, [panel("left-top", true)])).toBe(true)
    })
})
