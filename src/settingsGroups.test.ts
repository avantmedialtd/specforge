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
        expect([...SETTINGS_GROUPS]).toEqual([
            "workspaces",
            "appearance",
            "layout",
            "integrations",
            "identity",
            "desktop",
        ])
    })

    test("each carries its label", () => {
        expect(SETTINGS_GROUPS.map((group) => SETTINGS_GROUP_LABELS[group])).toEqual([
            "Workspaces",
            "Appearance",
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
        expect(isSettingsGroup("theme")).toBe(false)
        // An inherited property name is not a group either.
        expect(isSettingsGroup("toString")).toBe(false)
    })
})

describe("groupsForHost", () => {
    test("the desktop offers all six, in order", () => {
        expect(groupsForHost(DESKTOP)).toEqual([...SETTINGS_GROUPS])
    })

    test("the browser omits only Desktop app", () => {
        expect(groupsForHost(BROWSER)).toEqual([
            "workspaces",
            "appearance",
            "layout",
            "integrations",
            "identity",
        ])
    })

    test("Integrations stays between Layout and Identity on both hosts", () => {
        for (const host of [DESKTOP, BROWSER]) {
            const groups = groupsForHost(host)
            expect(groups).toContain("integrations")
            expect(groups.indexOf("integrations")).toBe(groups.indexOf("layout") + 1)
            expect(groups.indexOf("integrations")).toBe(groups.indexOf("identity") - 1)
        }
    })

    // `settings-view`: *Appearance is offered in the browser skin* — the
    // appearance is expressed entirely in the served stylesheets, so it
    // applies on both hosts, second after Workspaces.
    test("Appearance is offered on both hosts, between Workspaces and Layout", () => {
        for (const host of [DESKTOP, BROWSER]) {
            expect(groupsForHost(host).slice(0, 3)).toEqual(["workspaces", "appearance", "layout"])
            expect(effectiveSettingsGroup("appearance", host)).toBe("appearance")
        }
        expect(isSettingsGroup("appearance")).toBe(true)
    })

    test("the browser still offers Integrations for its non-secret settings", () => {
        // Its credential editors are desktop-only, but its switches, panel
        // slots and skip patterns keep the group applicable (`web-ui`:
        // *Desktop-Only Settings Are Hidden in the Web UI*).
        expect(groupsForHost(BROWSER)).toContain("integrations")
        expect(groupsForHost(BROWSER)).not.toContain("desktop")
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
