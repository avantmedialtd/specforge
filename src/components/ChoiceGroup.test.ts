import { describe, expect, test } from "bun:test"
import { choiceKeyTarget, choiceTabStop } from "./ChoiceGroup"

describe("choiceKeyTarget", () => {
    test("ArrowRight and ArrowDown select the next option, wrapping at the end", () => {
        for (const key of ["ArrowRight", "ArrowDown"]) {
            expect(choiceKeyTarget(key, 0, 3)).toBe(1)
            expect(choiceKeyTarget(key, 1, 3)).toBe(2)
            expect(choiceKeyTarget(key, 2, 3)).toBe(0)
            expect(choiceKeyTarget(key, 0, 2)).toBe(1)
            expect(choiceKeyTarget(key, 1, 2)).toBe(0)
        }
    })

    test("ArrowLeft and ArrowUp select the previous option, wrapping at the start", () => {
        for (const key of ["ArrowLeft", "ArrowUp"]) {
            expect(choiceKeyTarget(key, 2, 3)).toBe(1)
            expect(choiceKeyTarget(key, 1, 3)).toBe(0)
            expect(choiceKeyTarget(key, 0, 3)).toBe(2)
            expect(choiceKeyTarget(key, 1, 2)).toBe(0)
            expect(choiceKeyTarget(key, 0, 2)).toBe(1)
        }
    })

    test("other keys are ignored", () => {
        for (const key of [
            "Tab",
            "Enter",
            " ",
            "Home",
            "End",
            "Escape",
            "a",
            "Right",
            "arrowright",
        ]) {
            expect(choiceKeyTarget(key, 0, 2)).toBeNull()
            expect(choiceKeyTarget(key, 1, 3)).toBeNull()
        }
    })

    test("with nothing checked the arrows start from the matching end", () => {
        expect(choiceKeyTarget("ArrowRight", -1, 3)).toBe(0)
        expect(choiceKeyTarget("ArrowDown", -1, 3)).toBe(0)
        expect(choiceKeyTarget("ArrowLeft", -1, 3)).toBe(2)
        expect(choiceKeyTarget("ArrowUp", -1, 3)).toBe(2)
    })

    test("a group with no options has nowhere to go", () => {
        expect(choiceKeyTarget("ArrowRight", -1, 0)).toBeNull()
    })
})

describe("choiceTabStop", () => {
    test("the checked option is the group's single Tab stop", () => {
        expect(choiceTabStop(0, 2)).toBe(0)
        expect(choiceTabStop(1, 2)).toBe(1)
        expect(choiceTabStop(2, 3)).toBe(2)
    })

    test("with nothing checked the first option keeps the group reachable", () => {
        expect(choiceTabStop(-1, 2)).toBe(0)
    })
})

describe("the layout control is one Tab stop with wrapping arrows", () => {
    test("Unified, then ArrowRight to Side by side, then ArrowRight wraps back to Unified", () => {
        const options = ["unified", "split"]
        let checked = options.indexOf("unified")
        // Tab lands on "Unified" alone.
        expect(choiceTabStop(checked, options.length)).toBe(0)
        checked = choiceKeyTarget("ArrowRight", checked, options.length) ?? checked
        expect(options[checked]).toBe("split")
        expect(choiceTabStop(checked, options.length)).toBe(1)
        checked = choiceKeyTarget("ArrowRight", checked, options.length) ?? checked
        expect(options[checked]).toBe("unified")
        expect(choiceTabStop(checked, options.length)).toBe(0)
    })
})
