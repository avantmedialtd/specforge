import { describe, expect, test } from "bun:test"
import { horizontalOverflow, synchronizeCodeScroll } from "./diffHorizontalScroll"

describe("one horizontal range for every source line", () => {
    test("uses the longest overflow, including unequal viewport widths", () => {
        expect(horizontalOverflow([
            { content: 1500, viewport: 500 },
            { content: 300, viewport: 499 },
            { content: 1600.25, viewport: 500 },
        ])).toBe(1101)
        expect(horizontalOverflow([{ content: 20, viewport: 500 }])).toBe(0)
        expect(horizontalOverflow([])).toBe(0)
    })

    test("all old/new lines and the shared scrollbar move to the same column", () => {
        const ports = [{ scrollLeft: 0 }, { scrollLeft: 20 }, { scrollLeft: 0 }]
        expect(synchronizeCodeScroll(ports, 80, 400)).toBe(80)
        expect(ports.map((port) => port.scrollLeft)).toEqual([80, 80, 80])
        expect(synchronizeCodeScroll(ports, 1000, 400)).toBe(400)
        expect(ports.map((port) => port.scrollLeft)).toEqual([400, 400, 400])
        expect(synchronizeCodeScroll(ports, -20, 400)).toBe(0)
        expect(ports.map((port) => port.scrollLeft)).toEqual([0, 0, 0])
    })

    test("the resulting native scroll events make no redundant writes", () => {
        let writes = 0
        let value = 0
        const port = { get scrollLeft() { return value }, set scrollLeft(next) { value = next; writes++ } }
        synchronizeCodeScroll([port], 80, 400)
        synchronizeCodeScroll([port], 80, 400)
        expect(writes).toBe(1)
    })
})
