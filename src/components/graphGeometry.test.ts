import { describe, expect, test } from "bun:test"
import { computeGeometry, dayKey, ROW_H, SEP_H } from "./graphGeometry"

/// Local midday, so the assertions never straddle a day boundary through the
/// runner's time zone: `dayKey` is deliberately viewer-local.
function at(year: number, month1: number, day: number, hour = 12): { date: string } {
    return { date: new Date(year, month1 - 1, day, hour).toISOString() }
}

const label = (iso: string) => iso

describe("dayKey", () => {
    test("is 1-based in the month, so it reads as the month it means", () => {
        // `getMonth()` is 0-indexed; the naive spelling renders July as 6.
        expect(dayKey(at(2026, 7, 15).date)).toBe("2026-7-15")
        expect(dayKey(at(2026, 1, 1).date)).toBe("2026-1-1")
        expect(dayKey(at(2026, 12, 31).date)).toBe("2026-12-31")
    })

    test("an unparseable date falls back to the raw string", () => {
        expect(dayKey("not a date")).toBe("not a date")
    })

    test("two times on one local day share a key", () => {
        expect(dayKey(at(2026, 7, 15, 1).date)).toBe(dayKey(at(2026, 7, 15, 23).date))
    })
})

describe("computeGeometry", () => {
    test("one band per day, above that day's first commit", () => {
        const { separators, rowTop, totalHeight } = computeGeometry(
            [at(2026, 7, 15), at(2026, 7, 15), at(2026, 7, 14)],
            label,
        )
        expect(separators).toHaveLength(2)
        expect(rowTop).toEqual([SEP_H, SEP_H + ROW_H, SEP_H * 2 + ROW_H * 2])
        expect(totalHeight).toBe(SEP_H * 2 + ROW_H * 3)
    })

    test("the newest day gets a band too", () => {
        const { separators } = computeGeometry([at(2026, 7, 15)], label)
        expect(separators).toHaveLength(1)
        expect(separators[0]!.y).toBe(0)
    })

    /// The regression. `git log --all --date-order` puts the topological
    /// constraint first, so the date sequence is NOT monotonically decreasing:
    /// a branch's commits can carry the reader back to a day already passed and
    /// then forward again. Keying a band on its day alone then hands React two
    /// children with the same key — which is exactly what the console reported.
    test("a day that recurs non-adjacently gets distinct keys", () => {
        const { separators } = computeGeometry(
            [at(2026, 7, 15), at(2026, 7, 14), at(2026, 7, 15)],
            label,
        )
        expect(separators).toHaveLength(3)
        const keys = separators.map((s) => s.key)
        expect(new Set(keys).size).toBe(3)
        // Both bands still LABEL the same day — the fix is to the identity, not
        // to the grouping, because reordering commits is forbidden.
        expect(separators[0]!.label).toBe(separators[2]!.label)
    })

    test("keys are unique across a long interleaved history", () => {
        const commits = Array.from({ length: 60 }, (_, i) =>
            // Sawtooth: every third commit jumps back a day and then forward,
            // producing many non-adjacent repeats of the same few days.
            at(2026, 7, 10 + (i % 3)),
        )
        const { separators } = computeGeometry(commits, label)
        expect(new Set(separators.map((s) => s.key)).size).toBe(separators.length)
    })

    test("no commits means no bands and no height", () => {
        expect(computeGeometry([], label)).toEqual({
            rowTop: [],
            separators: [],
            totalHeight: 0,
        })
    })

    test("unparseable dates still group and still key uniquely", () => {
        const { separators } = computeGeometry(
            [{ date: "junk" }, { date: "junk" }, { date: "other" }],
            label,
        )
        expect(separators).toHaveLength(2)
        expect(new Set(separators.map((s) => s.key)).size).toBe(2)
    })
})
