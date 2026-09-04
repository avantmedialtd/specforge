import { describe, expect, test } from "bun:test"
import type { RepoBreakdown } from "../types"
import {
    BREAKDOWN_LIMIT,
    barPercent,
    capBreakdown,
    remainderLabel,
} from "./repoBreakdown"

/// One breakdown entry. Counts default to zero so a test names only the field
/// it is about.
function repo(label: string, activeCount = 0, archivedCount = 0): RepoBreakdown {
    return { label, activeCount, archivedCount }
}

/// `n` quiet entries, labelled `r0…r{n-1}` — filler for the cap's boundary.
function quiet(n: number): RepoBreakdown[] {
    return Array.from({ length: n }, (_, i) => repo(`r${i}`))
}

describe("capBreakdown", () => {
    test("shows every entry when the registry is smaller than the cap", () => {
        const capped = capBreakdown(quiet(3))
        expect(capped.shown).toHaveLength(3)
        expect(capped.hiddenCount).toBe(0)
        expect(capped.hiddenActiveCount).toBe(0)
    })

    // The boundary the cap is: exactly N is whole, N+1 withholds one. An
    // off-by-one in the slice moves exactly one of these.
    test("withholds nothing at exactly the cap", () => {
        const capped = capBreakdown(quiet(BREAKDOWN_LIMIT))
        expect(capped.shown).toHaveLength(BREAKDOWN_LIMIT)
        expect(capped.hiddenCount).toBe(0)
    })

    test("withholds one entry at one past the cap", () => {
        const capped = capBreakdown(quiet(BREAKDOWN_LIMIT + 1))
        expect(capped.shown).toHaveLength(BREAKDOWN_LIMIT)
        expect(capped.hiddenCount).toBe(1)
    })

    test("takes the front of the payload without re-sorting it", () => {
        // Arrives in the payload's order; the cap must not reorder it, or the
        // frontend would be silently repairing a backend contract.
        const ordered = [
            repo("meter-burn", 2, 57),
            repo("touchpoint", 1, 2),
            repo("pannonfox", 1, 0),
            repo("mushroom", 0, 276),
            repo("specforge", 0, 111),
            repo("artifex", 0, 90),
        ]
        expect(capBreakdown(ordered).shown.map((r) => r.label)).toEqual([
            "meter-burn",
            "touchpoint",
            "pannonfox",
            "mushroom",
            "specforge",
        ])
    })

    test("counts withheld entries that carry active work", () => {
        const capped = capBreakdown([...quiet(BREAKDOWN_LIMIT), repo("busy", 3), repo("idle")])
        expect(capped.hiddenCount).toBe(2)
        expect(capped.hiddenActiveCount).toBe(1)
    })
})

describe("remainderLabel", () => {
    test("renders no line when nothing was withheld", () => {
        expect(remainderLabel(capBreakdown(quiet(2)))).toBeNull()
    })

    test("reports that the cap hid no active work", () => {
        expect(remainderLabel(capBreakdown(quiet(BREAKDOWN_LIMIT + 9)))).toBe(
            "+ 9 more · none active",
        )
    })

    test("reports how much active work the cap hid", () => {
        const capped = capBreakdown([
            ...quiet(BREAKDOWN_LIMIT),
            repo("a", 1),
            repo("b", 4),
            repo("c"),
        ])
        expect(remainderLabel(capped)).toBe("+ 3 more · 2 active")
    })

    test("never restates the registry-wide archived total", () => {
        // The Dashboard's footnote already carries it; repeating it here would
        // say nothing new and would contradict the footnote once capped.
        const label = remainderLabel(capBreakdown([...quiet(BREAKDOWN_LIMIT), repo("big", 0, 276)]))
        expect(label).not.toContain("276")
        expect(label).not.toContain("archived")
    })
})

describe("barPercent", () => {
    test("fills the track for the largest active count", () => {
        expect(barPercent(2, 2)).toBe(100)
    })

    test("scales against the largest active count", () => {
        expect(barPercent(1, 2)).toBe(50)
        expect(barPercent(1, 4)).toBe(25)
    })

    test("returns zero for an entry with no active changes", () => {
        expect(barPercent(0, 4)).toBe(0)
    })

    test("does not divide by zero when nothing is in flight", () => {
        expect(barPercent(0, 0)).toBe(0)
    })
})
