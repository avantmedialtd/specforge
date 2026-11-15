import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act } from "react"
import { currentAppearance } from "../../appearance"
import { FakeElement, installFakeDom, type FakeDocument } from "../../testing/fakeDom"
import { FakeEventSource, installFakeTransport, settle } from "../../testing/fakeTransport"
import type { Appearance } from "../../types"
import { AppearanceGroup, THEME_CARDS } from "./AppearanceGroup"
import { groupsForHost } from "../../settingsGroups"
import { SettingsNav } from "./SettingsNav"

// `settings-view`: *The Appearance Group Offers Theme and Scheme*, *Settings
// Persist by One Rule*, *Settings Are Organised Into Groups*; `theme-system`:
// *A failed write restores the stored appearance*. Rendered through
// `react-dom/client` on the fake DOM, against the desktop's `invoke` and the
// fake web transport.

let dom: { document: FakeDocument; uninstall: () => void }
let createRoot: typeof import("react-dom/client").createRoot
const g = globalThis as unknown as Record<string, unknown>

beforeAll(async () => {
    dom = installFakeDom()
    g.IS_REACT_ACT_ENVIRONMENT = true
    createRoot = (await import("react-dom/client")).createRoot
})

afterAll(() => {
    delete g.IS_REACT_ACT_ENVIRONMENT
    dom.uninstall()
})

beforeEach(() => {
    delete (g.window as Record<string, unknown>).__TAURI_INTERNALS__
    delete (g.window as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__
})

interface Sent {
    command: string
    args: Record<string, unknown>
}

/// The native shell's `invoke`: the stored appearance as `stored`, and
/// `set_appearance` storing — or, while `failWrites` is set, refusing.
function installDesktop(stored: Appearance, options: { failWrites?: string } = {}) {
    const host = g.window as Record<string, unknown>
    const sent: Sent[] = []
    let callbackId = 0
    host.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    host.__TAURI_INTERNALS__ = {
        invoke: (command: string, args?: Record<string, unknown>) => {
            sent.push({ command, args: args ?? {} })
            if (command === "get_appearance") return Promise.resolve(stored)
            if (command === "set_appearance") {
                if (options.failWrites) return Promise.reject(options.failWrites)
                stored = args?.appearance as Appearance
                return Promise.resolve(null)
            }
            return Promise.resolve(callbackId)
        },
        transformCallback: () => {
            callbackId += 1
            return callbackId
        },
    }
    return {
        calls: (command: string) => sent.filter((call) => call.command === command),
        uninstall() {
            delete host.__TAURI_INTERNALS__
            delete host.__TAURI_EVENT_PLUGIN_INTERNALS__
        },
    }
}

function reactProps(element: FakeElement): Record<string, unknown> {
    const holder = element as unknown as Record<string, unknown>
    for (const key of Reflect.ownKeys(element)) {
        if (typeof key === "string" && key.startsWith("__reactProps$")) {
            return holder[key] as Record<string, unknown>
        }
    }
    throw new Error("the element carries no React props")
}

async function click(element: FakeElement): Promise<void> {
    const onClick = reactProps(element).onClick
    if (typeof onClick !== "function") throw new Error("the element has no click handler")
    await act(async () => {
        ;(onClick as (event: unknown) => void)({ preventDefault() {} })
    })
}

function card(container: FakeElement, theme: string): FakeElement {
    const found = container
        .findAll("button")
        .find((button) => button.getAttribute("aria-labelledby") === `theme-card-${theme}-name`)
    if (!found) throw new Error(`expected the ${theme} card`)
    return found
}

function choice(container: FakeElement, label: string): FakeElement {
    const found = container
        .findAll("button")
        .filter((button) => button.getAttribute("role") === "radio" && button.textContent === label)
    if (found.length !== 1) throw new Error(`expected one ${label} choice`)
    return found[0]!
}

function pressed(container: FakeElement): string[] {
    return THEME_CARDS.filter((entry) => card(container, entry.id).getAttribute("aria-pressed") === "true").map(
        (entry) => entry.id,
    )
}

function stamp(): [string | null, string | null] {
    const root = dom.document.documentElement
    return [root.getAttribute("data-theme"), root.getAttribute("data-scheme")]
}

async function mount() {
    const container = dom.document.createElement("div") as unknown as FakeElement
    const root = createRoot(container as unknown as Element)
    await act(async () => root.render(<AppearanceGroup />))
    await settle()
    return {
        container,
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

describe("the Appearance group", () => {
    test("offers the four themes in order, each previewed in its own theme, the stored one pressed", async () => {
        const desktop = installDesktop({ theme: "blueprint", scheme: "light" })
        const mounted = await mount()
        try {
            const cards = THEME_CARDS.map((entry) => card(mounted.container, entry.id))
            expect(THEME_CARDS.map((entry) => entry.name)).toEqual(["Forge", "Blueprint", "Folio", "Mycelium"])
            // Each preview names its own theme and the scheme the window is
            // resolved to (`settings-view`: *A card previews its own theme*).
            const previews = mounted.container
                .findAll("span")
                .filter((span) => span.getAttribute("class") === "theme-card-preview")
            expect(previews.map((preview) => preview.getAttribute("data-theme"))).toEqual([
                "forge",
                "blueprint",
                "folio",
                "mycelium",
            ])
            expect(previews.every((preview) => preview.getAttribute("data-scheme") === "light")).toBe(true)
            // The stored appearance came from the authoritative read.
            expect(pressed(mounted.container)).toEqual(["blueprint"])
            expect(cards.length).toBe(4)
            expect(stamp()).toEqual(["blueprint", "light"])
            expect(desktop.calls("get_appearance")).toHaveLength(1)
            // Named by its theme alone, with its blurb as the description.
            expect(card(mounted.container, "folio").getAttribute("aria-describedby")).toBe("theme-card-folio-blurb")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("activating a card applies it at once and stores it with one write", async () => {
        const desktop = installDesktop({ theme: "forge", scheme: "system" })
        const mounted = await mount()
        try {
            await click(card(mounted.container, "folio"))
            await settle()
            expect(desktop.calls("set_appearance").map((call) => call.args)).toEqual([
                { appearance: { theme: "folio", scheme: "system" } },
            ])
            expect(pressed(mounted.container)).toEqual(["folio"])
            expect(stamp()[0]).toBe("folio")
            // Activating the card already chosen writes nothing more.
            await click(card(mounted.container, "folio"))
            await settle()
            expect(desktop.calls("set_appearance")).toHaveLength(1)
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a failed write is reported beside the cards and the stored theme comes back", async () => {
        const desktop = installDesktop({ theme: "forge", scheme: "dark" }, { failWrites: "the settings file is read-only" })
        const mounted = await mount()
        try {
            expect(stamp()).toEqual(["forge", "dark"])
            await click(card(mounted.container, "mycelium"))
            await settle()
            expect(desktop.calls("set_appearance")).toHaveLength(1)
            expect(mounted.container.textContent).toContain(
                "Couldn't save the theme — the settings file is read-only",
            )
            expect(pressed(mounted.container)).toEqual(["forge"])
            expect(stamp()).toEqual(["forge", "dark"])
            expect(currentAppearance().theme).toBe("forge")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("choosing a scheme stores it beside the theme, and a failed one comes back with its own error", async () => {
        const desktop = installDesktop({ theme: "mycelium", scheme: "system" })
        const mounted = await mount()
        try {
            await click(choice(mounted.container, "Dark"))
            await settle()
            expect(desktop.calls("set_appearance").map((call) => call.args)).toEqual([
                { appearance: { theme: "mycelium", scheme: "dark" } },
            ])
            expect(stamp()).toEqual(["mycelium", "dark"])
            expect(choice(mounted.container, "Dark").getAttribute("aria-checked")).toBe("true")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }

        const failing = installDesktop({ theme: "mycelium", scheme: "dark" }, { failWrites: "disk full" })
        const again = await mount()
        try {
            await click(choice(again.container, "Light"))
            await settle()
            expect(again.container.textContent).toContain("Couldn't save the scheme — disk full")
            expect(stamp()).toEqual(["mycelium", "dark"])
            expect(choice(again.container, "Dark").getAttribute("aria-checked")).toBe("true")
        } finally {
            await again.unmount()
            failing.uninstall()
        }
    })

    // `settings-view`: *Appearance is offered in the browser skin* — the
    // controls are operable over the web transport, and a change made in
    // another window arrives as the service's notice.
    test("the browser skin stores a choice and adopts one made elsewhere", async () => {
        let stored: Appearance = { theme: "forge", scheme: "light" }
        const writes: unknown[] = []
        const transport = installFakeTransport((command, args) => {
            if (command === "get_appearance") return stored
            if (command === "set_appearance") {
                writes.push(args)
                stored = args.appearance as Appearance
            }
            return null
        })
        const mounted = await mount()
        try {
            await click(card(mounted.container, "blueprint"))
            await settle()
            expect(writes).toEqual([{ appearance: { theme: "blueprint", scheme: "light" } }])
            expect(stamp()).toEqual(["blueprint", "light"])

            await act(async () => {
                FakeEventSource.current?.emit("appearance-changed", { theme: "folio", scheme: "dark" })
            })
            await settle()
            expect(stamp()).toEqual(["folio", "dark"])
            expect(pressed(mounted.container)).toEqual(["folio"])
        } finally {
            await mounted.unmount()
            transport.uninstall()
        }
    })
})

describe("the group navigation", () => {
    // Rendered to markup: the fake DOM has no `<select>`.
    test("offers Appearance second, after Workspaces, on both hosts", async () => {
        const { renderToStaticMarkup } = await import("react-dom/server")
        for (const host of [{ desktop: true }, { desktop: false }]) {
            const markup = renderToStaticMarkup(
                <SettingsNav groups={groupsForHost(host)} current="appearance" onSelect={() => {}} />,
            )
            const items = [...markup.matchAll(/<button[^>]*class="settings-nav-item"[^>]*>([^<]*)<\/button>/g)].map(
                (match) => match[1],
            )
            expect(items.slice(0, 3)).toEqual(["Workspaces", "Appearance", "Layout"])
            expect(markup).toContain('aria-current="page">Appearance<')
        }
    })
})
