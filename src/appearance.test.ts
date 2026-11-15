import { describe, expect, test } from "bun:test"
import {
    APPEARANCE_STORAGE_KEY,
    DEFAULT_APPEARANCE,
    SCHEMES,
    THEMES,
    applyAppearance,
    currentAppearance,
    normalizeAppearance,
    readMirroredAppearance,
    resolveScheme,
    subscribeAppearance,
    writeMirroredAppearance,
    type AppearanceStore,
    type StampTarget,
} from "./appearance"

/// A `Storage` stand-in backed by a map.
function memoryStore(initial: Record<string, string> = {}): AppearanceStore & {
    items: Map<string, string>
} {
    const items = new Map(Object.entries(initial))
    return {
        items,
        getItem: (key) => items.get(key) ?? null,
        setItem: (key, value) => {
            items.set(key, value)
        },
    }
}

/// The part of `<html>` the stamp writes.
function fakeRoot(): StampTarget & { attributes: Map<string, string> } {
    const attributes = new Map<string, string>()
    return {
        attributes,
        setAttribute: (name, value) => {
            attributes.set(name, value)
        },
        style: {},
    }
}

describe("the catalogue", () => {
    test("offers the four themes and three schemes in Settings' order, Forge and System first", () => {
        expect([...THEMES]).toEqual(["forge", "blueprint", "folio", "mycelium"])
        expect([...SCHEMES]).toEqual(["system", "light", "dark"])
        expect(DEFAULT_APPEARANCE).toEqual({ theme: "forge", scheme: "system" })
    })
})

describe("resolveScheme", () => {
    // `theme-system`: *System follows the operating system*, *An explicit
    // scheme overrides the operating system*.
    test("System follows what the operating system reports", () => {
        expect(resolveScheme("system", true)).toBe("dark")
        expect(resolveScheme("system", false)).toBe("light")
    })

    test("Light and Dark override it either way", () => {
        for (const prefersDark of [true, false]) {
            expect(resolveScheme("light", prefersDark)).toBe("light")
            expect(resolveScheme("dark", prefersDark)).toBe("dark")
        }
    })
})

describe("normalizeAppearance", () => {
    test("keeps every known pair", () => {
        for (const theme of THEMES) {
            for (const scheme of SCHEMES) {
                expect(normalizeAppearance({ theme, scheme })).toEqual({ theme, scheme })
            }
        }
    })

    // `theme-system`: *An unrecognised stored theme degrades to the default* —
    // half by half, as the backend folds a settings file.
    test("folds an unknown half to its default and keeps the other", () => {
        expect(normalizeAppearance({ theme: "neon", scheme: "dark" })).toEqual({
            theme: "forge",
            scheme: "dark",
        })
        expect(normalizeAppearance({ theme: "mycelium", scheme: "dusk" })).toEqual({
            theme: "mycelium",
            scheme: "system",
        })
        expect(normalizeAppearance({ theme: "constructor", scheme: "toString" })).toEqual(
            DEFAULT_APPEARANCE,
        )
    })

    // Mycelium was called Classic, then Ink, before it shipped. No released
    // build stored either name, so neither is an alias: each folds like any
    // unknown name.
    test("folds a stale classic or ink, the old names of Mycelium, to Forge", () => {
        for (const stale of ["classic", "ink"]) {
            expect(normalizeAppearance({ theme: stale, scheme: "dark" })).toEqual({
                theme: "forge",
                scheme: "dark",
            })
        }
    })

    test("reads anything that is not an object as the default pair", () => {
        for (const value of [undefined, null, "folio", 3, ["folio", "dark"]]) {
            expect(normalizeAppearance(value)).toEqual(DEFAULT_APPEARANCE)
        }
    })
})

describe("the first-paint mirror", () => {
    test("round-trips a written appearance", () => {
        const store = memoryStore()
        writeMirroredAppearance({ theme: "folio", scheme: "dark" }, store)
        expect(store.items.get(APPEARANCE_STORAGE_KEY)).toBe('{"theme":"folio","scheme":"dark"}')
        expect(readMirroredAppearance(store)).toEqual({ theme: "folio", scheme: "dark" })
    })

    // `theme-system`: *A window with no applied appearance still renders* — in
    // Forge, following the operating system.
    test("an absent, unparseable or unreadable mirror yields Forge and System", () => {
        expect(readMirroredAppearance(memoryStore())).toEqual(DEFAULT_APPEARANCE)
        expect(readMirroredAppearance(null)).toEqual(DEFAULT_APPEARANCE)
        expect(
            readMirroredAppearance(memoryStore({ [APPEARANCE_STORAGE_KEY]: "{not json" })),
        ).toEqual(DEFAULT_APPEARANCE)
        expect(
            readMirroredAppearance(memoryStore({ [APPEARANCE_STORAGE_KEY]: '"mycelium"' })),
        ).toEqual(DEFAULT_APPEARANCE)
        const throwing: AppearanceStore = {
            getItem: () => {
                throw new Error("site data is blocked")
            },
            setItem: () => {
                throw new Error("site data is blocked")
            },
        }
        expect(readMirroredAppearance(throwing)).toEqual(DEFAULT_APPEARANCE)
        expect(() => writeMirroredAppearance({ theme: "mycelium", scheme: "light" }, throwing)).not.toThrow()
    })

    test("a mirror with an unknown half keeps the half it knows", () => {
        const store = memoryStore({
            [APPEARANCE_STORAGE_KEY]: '{"theme":"blueprint","scheme":"twilight"}',
        })
        expect(readMirroredAppearance(store)).toEqual({ theme: "blueprint", scheme: "system" })
    })
})

describe("applyAppearance", () => {
    test("stamps the theme and the RESOLVED scheme, sets color-scheme and writes the mirror", () => {
        const root = fakeRoot()
        const store = memoryStore()
        const applied = applyAppearance(
            { theme: "blueprint", scheme: "system" },
            { root, store, prefersDark: true },
        )
        expect(root.attributes.get("data-theme")).toBe("blueprint")
        expect(root.attributes.get("data-scheme")).toBe("dark")
        expect(root.style.colorScheme).toBe("dark")
        expect(readMirroredAppearance(store)).toEqual({ theme: "blueprint", scheme: "system" })
        expect(applied).toEqual({ theme: "blueprint", scheme: "system", resolvedScheme: "dark" })
        expect(currentAppearance()).toEqual(applied)
    })

    test("CSS never sees system: an explicit scheme stamps itself whatever the OS reports", () => {
        const root = fakeRoot()
        applyAppearance({ theme: "mycelium", scheme: "light" }, { root, store: null, prefersDark: true })
        expect(root.attributes.get("data-scheme")).toBe("light")
        applyAppearance({ theme: "mycelium", scheme: "system" }, { root, store: null, prefersDark: false })
        expect(root.attributes.get("data-scheme")).toBe("light")
        applyAppearance({ theme: "mycelium", scheme: "system" }, { root, store: null, prefersDark: true })
        expect(root.attributes.get("data-scheme")).toBe("dark")
    })

    test("stamps an unrecognised value as the default rather than as itself", () => {
        const root = fakeRoot()
        applyAppearance({ theme: "neon" as never, scheme: "dark" }, { root, store: null, prefersDark: false })
        expect(root.attributes.get("data-theme")).toBe("forge")
        expect(root.attributes.get("data-scheme")).toBe("dark")
    })

    test("tells subscribers once per change, and not for a stamp that changes nothing", () => {
        const root = fakeRoot()
        applyAppearance({ theme: "forge", scheme: "light" }, { root, store: null, prefersDark: false })
        let heard = 0
        const snapshots: unknown[] = []
        const unsubscribe = subscribeAppearance(() => {
            heard += 1
            snapshots.push(currentAppearance())
        })
        try {
            applyAppearance({ theme: "folio", scheme: "light" }, { root, store: null, prefersDark: false })
            applyAppearance({ theme: "folio", scheme: "light" }, { root, store: null, prefersDark: false })
            // Only the resolution moved: System now resolving to dark.
            applyAppearance({ theme: "folio", scheme: "system" }, { root, store: null, prefersDark: false })
            applyAppearance({ theme: "folio", scheme: "system" }, { root, store: null, prefersDark: true })
            expect(heard).toBe(3)
            expect(snapshots).toEqual([
                { theme: "folio", scheme: "light", resolvedScheme: "light" },
                { theme: "folio", scheme: "system", resolvedScheme: "light" },
                { theme: "folio", scheme: "system", resolvedScheme: "dark" },
            ])
        } finally {
            unsubscribe()
        }
        applyAppearance({ theme: "mycelium", scheme: "dark" }, { root, store: null, prefersDark: false })
        expect(heard).toBe(3)
    })

    test("keeps one snapshot object between changes, as useSyncExternalStore requires", () => {
        const root = fakeRoot()
        applyAppearance({ theme: "forge", scheme: "dark" }, { root, store: null, prefersDark: false })
        const first = currentAppearance()
        applyAppearance({ theme: "forge", scheme: "dark" }, { root, store: null, prefersDark: true })
        expect(currentAppearance()).toBe(first)
    })
})
