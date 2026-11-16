import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { runInNewContext } from "node:vm"
import {
    APPEARANCE_STORAGE_KEY,
    DARK_SCHEME_QUERY,
    DEFAULT_APPEARANCE,
    SCHEMES,
    THEMES,
    applyAppearance,
    readMirroredAppearance,
    type StampTarget,
} from "./appearance"

// `theme-system`: *The Appearance Is In Effect On the First Paint*; design D14.
// index.html stamps the root from an inline script in its <head>, before the
// bundle exists, so the storage key, the theme and scheme ids, the default
// pair and the media query in it are hand-written copies of this module's.
// These tests hold the copies equal, and run the script against
// `readMirroredAppearance` + `applyAppearance` in every environment the module
// tolerates, so the first stamp and `main.tsx`'s second one cannot disagree —
// a disagreement would itself be a repaint on every cold start.

const INDEX_HTML = readFileSync(fileURLToPath(new URL("../index.html", import.meta.url)), "utf8")

/// Every inline non-module script in the document: `<script>` with no attributes.
const INLINE_SCRIPTS = [...INDEX_HTML.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(
    (match) => match[1] ?? "",
)
const STAMP = INLINE_SCRIPTS[0] ?? ""

/// The JSON literal the first capture group of `pattern` finds in the stamp.
function literal(pattern: RegExp): unknown {
    const match = pattern.exec(STAMP)
    if (!match?.[1]) { throw new Error(`index.html's stamp no longer matches ${pattern}`) }
    return JSON.parse(match[1])
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

function stampOf(root: ReturnType<typeof fakeRoot>) {
    return {
        theme: root.attributes.get("data-theme") ?? null,
        scheme: root.attributes.get("data-scheme") ?? null,
        colorScheme: root.style.colorScheme ?? null,
    }
}

/// Run index.html's script as the browser runs it: as a non-module script, in a
/// fresh context whose `window` is this test's global object, so it reads
/// whatever the test installed there, and whose `document` holds the root it
/// stamps. The script is the checked-in text, run verbatim.
function runStamp(root: StampTarget): void {
    runInNewContext(STAMP, { window: globalThis, document: { documentElement: root } })
}

type StorageCase =
    | { kind: "holds"; raw: string | null }
    | { kind: "refuses reads" }
    | { kind: "refuses access" }
    | { kind: "missing" }

type MediaCase = { kind: "reports"; dark: boolean } | { kind: "throws" } | { kind: "missing" }

/// Install a storage and a media query on the global object, where both the
/// script and the module's ambient reads find them; returns the restorer.
/// Each answers only for the exact key and query, so a copy that drifted from
/// the module reads nothing and the comparison fails.
function installEnvironment(storage: StorageCase, media: MediaCase): () => void {
    const g = globalThis as Record<string, unknown>
    const saved = (["localStorage", "matchMedia"] as const).map(
        (name) => [name, Object.getOwnPropertyDescriptor(g, name)] as const,
    )
    const define = (name: string, descriptor: PropertyDescriptor) =>
        Object.defineProperty(g, name, { configurable: true, ...descriptor })

    if (storage.kind === "holds") {
        define("localStorage", {
            writable: true,
            value: {
                getItem: (key: string) => (key === APPEARANCE_STORAGE_KEY ? storage.raw : null),
                setItem: () => {},
            },
        })
    } else if (storage.kind === "refuses reads") {
        const refuse = () => {
            throw new Error("site data is blocked")
        }
        define("localStorage", { writable: true, value: { getItem: refuse, setItem: refuse } })
    } else if (storage.kind === "refuses access") {
        define("localStorage", {
            get: () => {
                throw new Error("SecurityError: access to storage is denied")
            },
        })
    } else {
        define("localStorage", { writable: true, value: undefined })
    }

    if (media.kind === "reports") {
        define("matchMedia", {
            writable: true,
            value: (query: string) => ({ matches: query === DARK_SCHEME_QUERY && media.dark }),
        })
    } else if (media.kind === "throws") {
        define("matchMedia", {
            writable: true,
            value: () => {
                throw new Error("matchMedia is unavailable")
            },
        })
    } else {
        define("matchMedia", { writable: true, value: undefined })
    }

    return () => {
        for (const [name, descriptor] of saved) {
            if (descriptor) { Object.defineProperty(g, name, descriptor) }
            else { delete g[name] }
        }
    }
}

/// The stored values a mirror can hold: absent, unparseable, the wrong shape,
/// unknown or inherited names in either half, and every real pair.
const MIRRORS: (string | null)[] = [
    null,
    "",
    "{not json",
    '"mycelium"',
    "3",
    "null",
    "true",
    '["folio","dark"]',
    '{"scheme":"light"}',
    '{"theme":"neon","scheme":"dark"}',
    '{"theme":"mycelium","scheme":"dusk"}',
    // Mycelium's earlier names, before it shipped: unknown now, no alias.
    '{"theme":"classic","scheme":"dark"}',
    '{"theme":"ink","scheme":"dark"}',
    '{"theme":"constructor","scheme":"toString"}',
    '{"theme":{"id":"folio"},"scheme":["dark"]}',
    '{"__proto__":{"theme":"folio","scheme":"dark"}}',
    ...THEMES.flatMap((theme) => SCHEMES.map((scheme) => JSON.stringify({ theme, scheme }))),
]

const STORAGE_CASES: StorageCase[] = [
    ...MIRRORS.map((raw): StorageCase => ({ kind: "holds", raw })),
    { kind: "refuses reads" },
    { kind: "refuses access" },
    { kind: "missing" },
]

const MEDIA_CASES: MediaCase[] = [
    { kind: "reports", dark: true },
    { kind: "reports", dark: false },
    { kind: "throws" },
    { kind: "missing" },
]

describe("index.html's first-paint stamp", () => {
    test("is the document's one inline non-module script, in <head>, ahead of anything that styles the page", () => {
        expect(INLINE_SCRIPTS).toHaveLength(1)
        const at = INDEX_HTML.indexOf("<script>")
        const head = INDEX_HTML.slice(0, INDEX_HTML.indexOf("</head>"))
        // The build appends its stylesheet link and module script just before
        // </head>, so a stamp inside <head> with no stylesheet ahead of it
        // runs before the stylesheet can apply.
        expect(at).toBeGreaterThan(-1)
        expect(at).toBeLessThan(head.length)
        expect(INDEX_HTML.slice(0, at)).not.toMatch(/rel="stylesheet"|<style\b|<script\b/)
    })

    test("names the module's storage key, ids, default pair and media query", () => {
        expect(literal(/getItem\(("[^"]*")\)/)).toBe(APPEARANCE_STORAGE_KEY)
        expect(literal(/var themes = (\[[^\]]*\]);/)).toEqual([...THEMES])
        expect(literal(/var schemes = (\[[^\]]*\]);/)).toEqual([...SCHEMES])
        expect(literal(/var theme = ("[^"]*");/)).toBe(DEFAULT_APPEARANCE.theme)
        expect(literal(/var scheme = ("[^"]*");/)).toBe(DEFAULT_APPEARANCE.scheme)
        expect(literal(/matchMedia\(("[^"]*")\)/)).toBe(DARK_SCHEME_QUERY)
    })

    test("stamps exactly what main.tsx then stamps, for every mirror, storage and media state", () => {
        let compared = 0
        for (const storage of STORAGE_CASES) {
            for (const media of MEDIA_CASES) {
                const restore = installEnvironment(storage, media)
                try {
                    const first = fakeRoot()
                    runStamp(first)
                    const second = fakeRoot()
                    // `main.tsx`'s call, with its ambient reads of the same
                    // environment; `store: null` keeps it from writing back.
                    applyAppearance(readMirroredAppearance(), { root: second, store: null })
                    expect({ storage, media, stamp: stampOf(first) }).toEqual({
                        storage,
                        media,
                        stamp: stampOf(second),
                    })
                    compared += 1
                } finally {
                    restore()
                }
            }
        }
        expect(compared).toBe(STORAGE_CASES.length * MEDIA_CASES.length)
    })

    // `theme-system`: *A cold start does not flash the default* — the case the
    // parent's smoke caught: Mycelium, Dark stored, a light operating system.
    test("stamps a stored Mycelium in Dark under a light OS, a stale classic or ink as Forge, and System as the OS reports", () => {
        for (const [raw, dark, expected] of [
            ['{"theme":"mycelium","scheme":"dark"}', false, { theme: "mycelium", scheme: "dark" }],
            // Stale `classic` and `ink` — Mycelium's names before it shipped — are unknown.
            ['{"theme":"classic","scheme":"dark"}', false, { theme: "forge", scheme: "dark" }],
            ['{"theme":"ink","scheme":"dark"}', false, { theme: "forge", scheme: "dark" }],
            ['{"theme":"folio","scheme":"system"}', true, { theme: "folio", scheme: "dark" }],
            ['{"theme":"folio","scheme":"system"}', false, { theme: "folio", scheme: "light" }],
        ] as const) {
            const restore = installEnvironment({ kind: "holds", raw }, { kind: "reports", dark })
            try {
                const root = fakeRoot()
                runStamp(root)
                expect(stampOf(root)).toEqual({ ...expected, colorScheme: expected.scheme })
            } finally {
                restore()
            }
        }
    })

    test("never throws, even when the root refuses the stamp", () => {
        const restore = installEnvironment({ kind: "refuses access" }, { kind: "throws" })
        try {
            const refusing: StampTarget = {
                setAttribute: () => {
                    throw new Error("the root is read-only")
                },
                style: {},
            }
            expect(() => runStamp(refusing)).not.toThrow()
        } finally {
            restore()
        }
    })
})
