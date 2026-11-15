import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import {
    CONTRAST_PAIRS,
    LIGHTNESS_PAIRS,
    LIGHTNESS_STEP,
    contrastRatio,
    darkSelector,
    lightSelector,
    lightness,
    measure,
    parseBlocks,
    parseColor,
    parseDeclarations,
    resolveValue,
    themeTokens,
    tokenColor,
    type Scheme,
    type TokenMap,
} from "./themeContrast"

// `theme-system`: *Every Theme Meets the Same Legibility Floors*;
// `visual-identity`: *Design Token Layer* — every theme defines the whole
// vocabulary. The floors are measured on the themes' own files, so a value
// edited in a stylesheet is the value measured here.

function sheet(relative: string): string {
    return readFileSync(fileURLToPath(new URL(`./styles/${relative}`, import.meta.url)), "utf8")
}

const THEME_IDS = ["forge", "blueprint", "folio", "mycelium"] as const
type ThemeId = (typeof THEME_IDS)[number]
const THEMES = Object.fromEntries(
    THEME_IDS.map((id) => [id, themeTokens(sheet(`themes/${id}.css`), id)]),
) as Record<ThemeId, { light: TokenMap; dark: TokenMap }>

const BASE = sheet("base.css")
const BASE_BLOCKS = parseBlocks(BASE)
/// The base sheet's own, theme-independent tokens: its first `:root` block
/// (the space scale, the reading width, the border width). The second is the
/// Forge fallback, which must not stand in for a token a theme forgets.
const SHARED = parseDeclarations(
    BASE_BLOCKS.find((block) => block.within === null && block.selector === ":root")!.body,
)

const SCHEMES: Scheme[] = ["light", "dark"]

/// One theme's failures in one scheme, each named by its pair.
function failures(tokens: TokenMap): string[] {
    const found: string[] = []
    for (const pair of CONTRAST_PAIRS) {
        const ratio = measure(pair, tokens, SHARED)
        if (ratio < pair.floor) {
            found.push(`${pair.fg} on ${pair.bg} (${pair.why}): ${ratio.toFixed(2)} < ${pair.floor}`)
        }
    }
    for (const [a, b] of LIGHTNESS_PAIRS) {
        const delta = Math.abs(lightness(tokenColor(a, tokens, SHARED)) - lightness(tokenColor(b, tokens, SHARED)))
        if (delta < LIGHTNESS_STEP) found.push(`L* ${a} vs ${b}: ${delta.toFixed(1)} < ${LIGHTNESS_STEP}`)
    }
    return found
}

/// The pairs Mycelium falls below. Mycelium keeps the colours the application rendered
/// before themes existed, and retuning them is a separate change (theme-system
/// design D14), so these are recorded rather than fixed, and pinned exactly as
/// a closed list: a Mycelium regression fails, and so does a retune that lifts one
/// of them without removing it from this list.
const MYCELIUM_BELOW_FLOOR: Record<Scheme, string[]> = {
    light: [
        "--text-faint on --surface-2",
        "--text-faint on --selection-fill",
        "--border-strong on --bg",
        "--border-strong on --surface",
        "--border-strong on --surface-2",
        "--border-strong on --surface-3",
        "L* --accent vs --ok-strong",
        "L* --accent vs --diff-removed-ink",
        "L* --ok-strong vs --diff-removed-ink",
    ],
    dark: [
        "--text-faint on --selection-fill",
        "--on-accent on --primary-fill-hover",
        "--on-accent on --primary-fill-active",
        "L* --accent vs --warn",
        "L* --accent vs --diff-removed-ink",
    ],
}

/// A failure's pair, without its measurement.
function pairOf(failure: string): string {
    return failure.replace(/ \(.*$/, "").replace(/: [\d.]+ < [\d.]+$/, "")
}

describe("every theme meets the legibility floors", () => {
    for (const theme of THEME_IDS) {
        for (const scheme of SCHEMES) {
            test(`${theme}, ${scheme}`, () => {
                const found = failures(THEMES[theme][scheme])
                if (theme === "mycelium") {
                    expect(found.map(pairOf)).toEqual(MYCELIUM_BELOW_FLOOR[scheme])
                } else {
                    expect(found).toEqual([])
                }
            })
        }
    }

    // `theme-system`: *A theme below a floor fails the suite*.
    test("a secondary-text tone lowered below 4.5:1 fails, naming the pair", () => {
        const lowered = { ...THEMES.forge.dark, "--text-muted": "#6a655d" }
        expect(failures(lowered)).toContain(
            "--text-muted on --surface (text on a plane): " +
                contrastRatio(parseColor("#6a655d")!, parseColor(THEMES.forge.dark["--surface"])!).toFixed(2) +
                " < 4.5",
        )
    })

    // `theme-system`: *The accent is not mistaken for an error*.
    test("Forge's accent and removal colour differ in lightness in both schemes", () => {
        for (const scheme of SCHEMES) {
            const tokens = THEMES.forge[scheme]
            const accent = lightness(tokenColor("--accent", tokens, SHARED))
            const removal = lightness(tokenColor("--diff-removed-ink", tokens, SHARED))
            expect(Math.abs(accent - removal)).toBeGreaterThanOrEqual(LIGHTNESS_STEP)
        }
    })
})

/// The token vocabulary the `visual-identity` capability names at minimum.
const SPEC_VOCABULARY = [
    "--bg",
    "--surface",
    "--surface-2",
    "--surface-3",
    "--border",
    "--border-strong",
    "--text",
    "--text-muted",
    "--text-faint",
    "--accent",
    "--accent-hover",
    "--accent-active",
    "--accent-strong",
    "--accent-tint",
    "--accent-tint-strong",
    "--accent-glow",
    "--ok",
    "--warn",
    "--border-hairline-top",
    "--shadow-0",
    "--shadow-1",
    "--shadow-2",
    "--shadow-3",
    "--shadow-accent",
    "--shadow-accent-strong",
    "--glow-ok",
    "--shadow-focus",
    "--sidebar-edge",
    "--text-xs",
    "--text-sm",
    "--text-base",
    "--text-md",
    "--text-lg",
    "--text-xl",
    "--text-2xl",
    "--font-ui",
    "--font-mono",
    "--leading-tight",
    "--leading-prose",
    "--leading-code",
    "--radius-sm",
    "--radius",
    "--radius-md",
]

describe("every theme defines the whole vocabulary", () => {
    const union = [...new Set(THEME_IDS.flatMap((id) => Object.keys(THEMES[id].light)))].sort()

    test("the vocabulary includes everything the visual-identity capability names", () => {
        for (const name of SPEC_VOCABULARY) expect(union).toContain(name)
    })

    for (const theme of THEME_IDS) {
        for (const scheme of SCHEMES) {
            // `visual-identity`: *Every theme defines the whole vocabulary* —
            // no rule resolves a token to nothing under any theme.
            test(`${theme}, ${scheme}: every token is defined and resolves`, () => {
                const tokens = THEMES[theme][scheme]
                expect(Object.keys(tokens).sort()).toEqual(union)
                for (const name of union) {
                    expect(() => resolveValue(tokens[name], tokens, SHARED)).not.toThrow()
                }
            })
        }
    }

    test("the space scale is the base sheet's, the same in every theme", () => {
        for (const step of [1, 2, 3, 4, 5, 6, 7]) expect(SHARED[`--space-${step}`]).toBeDefined()
        for (const theme of THEME_IDS) {
            expect(Object.keys(THEMES[theme].light).some((name) => name.startsWith("--space-"))).toBe(false)
        }
    })

    test("each theme writes its tokens as one light block and one dark block", () => {
        for (const theme of THEME_IDS) {
            const blocks = parseBlocks(sheet(`themes/${theme}.css`)).filter(
                (block) => block.selector === lightSelector(theme) || block.selector === darkSelector(theme),
            )
            expect(blocks.map((block) => block.selector)).toEqual([lightSelector(theme), darkSelector(theme)])
        }
    })

    // Every structural rule a theme writes is scoped to it, so no theme's
    // rule can reach a window in another (`theme-system`: *One Component
    // Tree Serves Every Theme*).
    test("every rule in a theme file is prefixed with that theme's attribute", () => {
        for (const theme of THEME_IDS) {
            for (const block of parseBlocks(sheet(`themes/${theme}.css`))) {
                for (const selector of block.selector.split(/,(?![^(]*\))/)) {
                    expect(selector.trim().startsWith(lightSelector(theme))).toBe(true)
                }
            }
        }
    })
})

describe("the base sheet's fallback", () => {
    // `theme-system`: *A window with no applied appearance still renders* —
    // in Forge, light or dark as the operating system reports. The fallback
    // is a copy, so this is what keeps it equal to forge.css.
    test("is Forge's light block on a bare :root", () => {
        const fallback = BASE_BLOCKS.filter((block) => block.within === null && block.selector === ":root")
        expect(fallback).toHaveLength(2)
        expect(parseDeclarations(fallback[1].body)).toEqual(THEMES.forge.light)
    })

    test("is Forge's dark overrides under the OS dark scheme, only while no scheme is stamped", () => {
        const dark = BASE_BLOCKS.filter((block) => block.within !== null)
            .filter((block) => block.within === "@media (prefers-color-scheme: dark)")
        expect(dark.map((block) => block.selector)).toEqual([":root:not([data-scheme])"])
        const forgeDark = parseBlocks(sheet("themes/forge.css")).find(
            (block) => block.selector === darkSelector("forge"),
        )!
        expect(parseDeclarations(dark[0].body)).toEqual(parseDeclarations(forgeDark.body))
    })

    test("is the only colour-scheme media query left in the app's stylesheets", () => {
        expect(BASE.match(/prefers-color-scheme/g)).toHaveLength(1)
        for (const theme of THEME_IDS) {
            expect(sheet(`themes/${theme}.css`)).not.toContain("prefers-color-scheme")
        }
    })

    // `visual-identity`: *Design Token Layer* — colour literals live only in
    // token definitions: none in a rule of the base sheet (the one functional
    // black is itself a token), none in a theme's structural rules.
    test("no rule outside a token block holds a colour literal", () => {
        const literal = /#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i
        const outsideTokens = (body: string) =>
            body
                .split(";")
                .filter((declaration) => !declaration.trim().startsWith("--"))
                .join(";")
        for (const block of BASE_BLOCKS) {
            if (block.selector.startsWith(":root")) continue
            const declarations = outsideTokens(block.body).replace(/var\([^)]*\)/g, "")
            const fallbackScrim = block.selector === ".figure-lightbox::backdrop"
            if (!fallbackScrim) expect(`${block.selector} { ${declarations} }`).not.toMatch(literal)
        }
        for (const theme of THEME_IDS) {
            for (const block of parseBlocks(sheet(`themes/${theme}.css`))) {
                expect(`${block.selector} { ${outsideTokens(block.body)} }`).not.toMatch(literal)
            }
        }
    })
})

/// What makes Mycelium as spacious as Forge (`visual-identity`: *Pinned Values
/// Describe the Mycelium Theme*, scenario *Mycelium is as spacious as Forge*): every
/// type-size, line-height and document-size token, and the row and control
/// spacing the themes supply. Mycelium's colours, depth, radii, label casing and
/// unboxed inline code (hence its `--code-pad` of 0) stay its own; these
/// may not drift from Forge's.
const SPACIOUS_TOKENS = [
    "--text-3xs",
    "--text-2xs",
    "--text-xs",
    "--text-sm",
    "--text-base",
    "--text-md",
    "--text-lg",
    "--text-xl",
    "--text-2xl",
    "--text-floor",
    "--leading-tight",
    "--leading-prose",
    "--leading-code",
    "--doc-size",
    "--doc-h1",
    "--doc-h2",
    "--doc-h3",
    "--doc-h4",
    "--code-size",
    "--chip-pad-x",
]

describe("Mycelium is as spacious as Forge", () => {
    for (const scheme of SCHEMES) {
        test(`every type-size, line-height and spacing token resolves to Forge's, ${scheme}`, () => {
            const resolved = (theme: ThemeId) =>
                SPACIOUS_TOKENS.map((name) => [name, resolveValue(THEMES[theme][scheme][name]!, THEMES[theme][scheme], SHARED)])
            expect(resolved("mycelium")).toEqual(resolved("forge"))
        })
    }

    // A size or leading token added to the vocabulary later must join the
    // list above, or Mycelium could quietly fall out of step with Forge.
    test("the list names every type-size, line-height and document-size token there is", () => {
        const sizeLike = Object.keys(THEMES.forge.light).filter((name) =>
            /^--(text-(\d?xs|sm|base|md|lg|\d?xl|floor)|leading-[a-z]+|doc-(size|h\d)|code-size)$/.test(name),
        )
        expect(sizeLike.sort()).toEqual(SPACIOUS_TOKENS.filter((name) => name !== "--chip-pad-x").sort())
    })

    test("nothing in Mycelium is set under 13px", () => {
        for (const scheme of SCHEMES) {
            for (const name of ["--text-3xs", "--text-2xs", "--text-xs", "--text-floor"]) {
                expect(resolveValue(THEMES.mycelium[scheme][name]!, THEMES.mycelium[scheme], SHARED)).toBe("13px")
            }
        }
    })

    test("Mycelium sets code and identifiers in Meslo, and keeps Inter for interface and prose", () => {
        for (const scheme of SCHEMES) {
            const tokens = THEMES.mycelium[scheme]
            expect(tokens["--font-mono"]).toStartWith('"Meslo LG M", Menlo,')
            expect(tokens["--font-ui"]).toStartWith('"Inter Variable",')
            expect(resolveValue(tokens["--font-doc"]!, tokens, SHARED)).toBe(tokens["--font-ui"])
        }
    })
})

describe("the parser and the arithmetic", () => {
    test("parses every colour syntax the themes use", () => {
        expect(parseColor("#fff")).toEqual({ r: 255, g: 255, b: 255, a: 1 })
        expect(parseColor("#141312")).toEqual({ r: 20, g: 19, b: 18, a: 1 })
        expect(parseColor("rgba(79, 91, 217, 0.1)")).toEqual({ r: 79, g: 91, b: 217, a: 0.1 })
        expect(parseColor("transparent")).toEqual({ r: 0, g: 0, b: 0, a: 0 })
        const hsl = parseColor("hsl(208 75% 44%)")!
        expect([Math.round(hsl.r), Math.round(hsl.g), Math.round(hsl.b)]).toEqual([28, 118, 196])
        expect(parseColor("var(--x)")).toBeNull()
    })

    test("measures WCAG's ratio: black on white is 21:1", () => {
        expect(contrastRatio(parseColor("#000")!, parseColor("#fff")!)).toBeCloseTo(21, 5)
    })

    test("resolves var() references through the theme, then the base sheet, then a fallback", () => {
        const tokens = { "--a": "var(--b)", "--b": "#123456", "--rule": "var(--border-width) solid var(--a)" }
        expect(resolveValue("var(--a)", tokens)).toBe("#123456")
        expect(resolveValue("var(--rule)", tokens, { "--border-width": "1px" })).toBe("1px solid #123456")
        expect(resolveValue("var(--missing, rgba(0, 0, 0, 0.5))", tokens)).toBe("rgba(0, 0, 0, 0.5)")
        expect(() => resolveValue("var(--missing)", tokens)).toThrow("--missing is not defined")
    })

    test("composites a translucent token over its backdrop before measuring", () => {
        const tokens = { "--wash": "rgba(0, 0, 0, 0.5)", "--ground": "#ffffff" }
        const wash = tokenColor("--wash", tokens, {}, parseColor("#ffffff")!)
        expect(Math.round(wash.r)).toBe(128)
        expect(() => tokenColor("--wash", tokens)).toThrow("translucent")
    })
})
