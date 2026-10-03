import { describe, expect, test } from "bun:test"
import {
    commitDecision,
    parseList,
    parsePollSeconds,
    parsePort,
    parseText,
    sameList,
} from "./settingsFields"

describe("parsePort", () => {
    test("accepts the range's edges", () => {
        expect(parsePort("1")).toEqual({ ok: true, value: 1 })
        expect(parsePort("65535")).toEqual({ ok: true, value: 65535 })
    })

    test("refuses just outside the range", () => {
        expect(parsePort("0").ok).toBe(false)
        expect(parsePort("65536").ok).toBe(false)
        expect(parsePort("70000").ok).toBe(false)
    })

    test("refuses anything that is not plain digits", () => {
        for (const raw of ["", "  ", "43.5", "4e3", "-80", "+80", "abc", "80a", "0x50"]) {
            expect(parsePort(raw).ok).toBe(false)
        }
    })

    test("names the range when it refuses", () => {
        const verdict = parsePort("70000")
        expect(verdict.ok).toBe(false)
        if (!verdict.ok) expect(verdict.message).toContain("65535")
    })

    test("ignores surrounding whitespace and leading zeros", () => {
        expect(parsePort(" 4317 ")).toEqual({ ok: true, value: 4317 })
        expect(parsePort("04317")).toEqual({ ok: true, value: 4317 })
    })
})

describe("parsePollSeconds", () => {
    test("accepts one second and more", () => {
        expect(parsePollSeconds("1")).toEqual({ ok: true, value: 1 })
        expect(parsePollSeconds("10")).toEqual({ ok: true, value: 10 })
    })

    test("refuses zero, fractions and non-numbers", () => {
        for (const raw of ["0", "", "1.5", "-1", "ten"]) {
            expect(parsePollSeconds(raw).ok).toBe(false)
        }
    })
})

describe("parseText", () => {
    test("trims, and reads an empty field as no value", () => {
        expect(parseText("  Ada ")).toEqual({ ok: true, value: "Ada" })
        expect(parseText("")).toEqual({ ok: true, value: null })
        expect(parseText("   ")).toEqual({ ok: true, value: null })
    })
})

describe("parseList and sameList", () => {
    test("split on commas, trim, and drop empty entries", () => {
        expect(parseList(" a@x.com, ,b@y.com,")).toEqual({ ok: true, value: ["a@x.com", "b@y.com"] })
        expect(parseList("")).toEqual({ ok: true, value: [] })
    })

    test("compare entries in order", () => {
        expect(sameList(["a", "b"], ["a", "b"])).toBe(true)
        expect(sameList(["a", "b"], ["b", "a"])).toBe(false)
        expect(sameList(["a"], ["a", "b"])).toBe(false)
        expect(sameList([], [])).toBe(true)
    })
})

describe("commitDecision", () => {
    test("an unparseable draft is invalid and carries the parser's message", () => {
        expect(commitDecision("70000", 4317, parsePort)).toEqual({
            kind: "invalid",
            message: "The port must be a whole number from 1 to 65535.",
        })
    })

    test("a draft that parses to the stored value writes nothing", () => {
        expect(commitDecision(" 4317 ", 4317, parsePort)).toEqual({ kind: "unchanged" })
        expect(commitDecision("", null, parseText)).toEqual({ kind: "unchanged" })
    })

    test("a draft that parses to a new value writes it", () => {
        expect(commitDecision("8080", 4317, parsePort)).toEqual({ kind: "write", value: 8080 })
        expect(commitDecision("", "Ada", parseText)).toEqual({ kind: "write", value: null })
    })

    test("lists compare by their entries through the equality given", () => {
        expect(commitDecision("a, b", ["a", "b"], parseList, sameList)).toEqual({ kind: "unchanged" })
        expect(commitDecision("a", ["a", "b"], parseList, sameList)).toEqual({
            kind: "write",
            value: ["a"],
        })
    })
})
