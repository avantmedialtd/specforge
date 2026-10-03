import { describe, expect, test } from "bun:test"
import {
    DEFAULT_SETTINGS_GROUP,
    SETTINGS_GROUPS,
    SETTINGS_GROUP_LABELS,
    effectiveSettingsGroup,
    groupsForHost,
    isSettingsGroup,
} from "./settingsGroups"

const DESKTOP = { desktop: true }
const BROWSER = { desktop: false }

describe("the settings groups", () => {
    test("are offered in a fixed order", () => {
        expect([...SETTINGS_GROUPS]).toEqual(["workspaces", "layout", "integrations", "identity", "desktop"])
    })

    test("each carries its label", () => {
        expect(SETTINGS_GROUPS.map((group) => SETTINGS_GROUP_LABELS[group])).toEqual([
            "Workspaces",
            "Layout",
            "Integrations",
            "Identity",
            "Desktop app",
        ])
    })

    test("Workspaces is the default", () => {
        expect(DEFAULT_SETTINGS_GROUP).toBe("workspaces")
    })
})

describe("isSettingsGroup", () => {
    test("knows every group", () => {
        for (const group of SETTINGS_GROUPS) expect(isSettingsGroup(group)).toBe(true)
    })

    test("rejects the empty string, a label's casing and unknown slugs", () => {
        expect(isSettingsGroup("")).toBe(false)
        expect(isSettingsGroup("Workspaces")).toBe(false)
        expect(isSettingsGroup("appearance")).toBe(false)
        // An inherited property name is not a group either.
        expect(isSettingsGroup("toString")).toBe(false)
    })
})

describe("groupsForHost", () => {
    test("the desktop offers all five, in order", () => {
        expect(groupsForHost(DESKTOP)).toEqual([...SETTINGS_GROUPS])
    })

    test("the browser omits only Desktop app", () => {
        expect(groupsForHost(BROWSER)).toEqual(["workspaces", "layout", "integrations", "identity"])
    })
})

describe("effectiveSettingsGroup", () => {
    test("keeps every group the desktop offers", () => {
        for (const group of SETTINGS_GROUPS) expect(effectiveSettingsGroup(group, DESKTOP)).toBe(group)
    })

    test("keeps every group the browser offers", () => {
        for (const group of groupsForHost(BROWSER)) {
            expect(effectiveSettingsGroup(group, BROWSER)).toBe(group)
        }
    })

    test("falls back to Workspaces for Desktop app in the browser", () => {
        expect(effectiveSettingsGroup("desktop", BROWSER)).toBe("workspaces")
    })
})
