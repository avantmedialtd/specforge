/// The themes' token blocks, read as data: a parser for the flat
/// `--name: value;` lists in `src/styles/themes/*.css`, per-theme per-scheme
/// resolution, and the WCAG 2.x arithmetic the contrast test holds every
/// palette to (`theme-system`: *Every Theme Meets the Same Legibility
/// Floors*; design D8).
///
/// The stylesheets stay the source of truth — no palette is restated here —
/// so the direct edit-and-see loop on a theme file is kept, and a value a
/// theme ships is the value the test measures. Pure: the test reads the files
/// and hands their text in.

/// A theme's custom properties in one scheme, values as written (`var()`
/// references unresolved).
export type TokenMap = Record<string, string>

export type Scheme = "light" | "dark"

/// One theme's tokens in both schemes: the light block, and the light block
/// overlaid with the dark block's overrides — what the root resolves when
/// `data-scheme="dark"` is stamped.
export interface ThemeTokens {
    light: TokenMap
    dark: TokenMap
}

/// A rule block: its selector and its body, comments removed.
export interface CssBlock {
    selector: string
    body: string
    /// The at-rule prelude the block sits inside, or null at top level.
    within: string | null
}

/// Comments out, so a `;` or `{` inside one is never read as syntax.
export function stripComments(css: string): string {
    return css.replace(/\/\*[\s\S]*?\*\//g, "")
}

/// Every rule block of `css`, top-level and one at-rule deep (the base sheet's
/// fallback sits inside `@media`). Enough for the theme files and the base
/// sheet's token blocks, which are flat by construction; not a CSS parser.
export function parseBlocks(css: string): CssBlock[] {
    const text = stripComments(css)
    const blocks: CssBlock[] = []
    const walk = (source: string, within: string | null) => {
        let i = 0
        while (i < source.length) {
            const open = source.indexOf("{", i)
            if (open < 0) { break }
            const prelude = source.slice(i, open).trim()
            let depth = 1
            let j = open + 1
            while (j < source.length && depth > 0) {
                if (source[j] === "{") { depth += 1 }
                else if (source[j] === "}") { depth -= 1 }
                j += 1
            }
            const body = source.slice(open + 1, j - 1)
            if (prelude.startsWith("@")) { walk(body, prelude) }
            else { blocks.push({ selector: prelude.replace(/\s+/g, " "), body, within }) }
            i = j
        }
    }
    walk(text, null)
    return blocks
}

/// The custom properties a block body declares, in order, later winning.
export function parseDeclarations(body: string): TokenMap {
    const tokens: TokenMap = {}
    for (const declaration of stripComments(body).split(";")) {
        const colon = declaration.indexOf(":")
        if (colon < 0) { continue }
        const name = declaration.slice(0, colon).trim()
        if (!name.startsWith("--")) { continue }
        tokens[name] = declaration.slice(colon + 1).replace(/\s+/g, " ").trim()
    }
    return tokens
}

/// The light selector of a theme's token block.
export function lightSelector(theme: string): string {
    return `[data-theme="${theme}"]`
}

/// The dark selector of a theme's token block.
export function darkSelector(theme: string): string {
    return `[data-theme="${theme}"][data-scheme="dark"]`
}

/// A theme's tokens in both schemes, from its file's text.
export function themeTokens(css: string, theme: string): ThemeTokens {
    const light: TokenMap = {}
    const darkOverrides: TokenMap = {}
    for (const block of parseBlocks(css)) {
        if (block.within !== null) { continue }
        if (block.selector === lightSelector(theme)) { Object.assign(light, parseDeclarations(block.body)) }
        else if (block.selector === darkSelector(theme)) {
            Object.assign(darkOverrides, parseDeclarations(block.body))
        }
    }
    return { light, dark: { ...light, ...darkOverrides } }
}

/// Substitute every `var(--name)` / `var(--name, fallback)` in `value` from
/// `tokens`, then from `shared` (the base sheet's theme-independent tokens),
/// recursively. An unresolvable reference without a fallback throws: a token
/// a theme leaves undefined is exactly what the vocabulary test exists to
/// catch, and silently measuring something else would hide it.
export function resolveValue(value: string, tokens: TokenMap, shared: TokenMap = {}, depth = 0): string {
    if (depth > 20) { throw new Error(`var() cycle resolving ${value}`) }
    return value.replace(/var\(\s*(--[a-z0-9-]+)\s*(?:,\s*([^()]*(?:\([^()]*\))?[^()]*))?\)/gi, (_, name: string, fallback?: string) => {
        const found = tokens[name] ?? shared[name]
        if (found !== undefined) { return resolveValue(found, tokens, shared, depth + 1) }
        if (fallback !== undefined) { return resolveValue(fallback.trim(), tokens, shared, depth + 1) }
        throw new Error(`${name} is not defined`)
    })
}

/// A colour, channels 0–255 and alpha 0–1.
export interface Rgba {
    r: number
    g: number
    b: number
    a: number
}

function clampByte(value: number): number {
    return Math.max(0, Math.min(255, value))
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
    const hue = ((h % 360) + 360) % 360
    const chroma = (1 - Math.abs(2 * l - 1)) * s
    const x = chroma * (1 - Math.abs(((hue / 60) % 2) - 1))
    const m = l - chroma / 2
    const [r, g, b] =
        hue < 60
            ? [chroma, x, 0]
            : hue < 120
              ? [x, chroma, 0]
              : hue < 180
                ? [0, chroma, x]
                : hue < 240
                  ? [0, x, chroma]
                  : hue < 300
                    ? [x, 0, chroma]
                    : [chroma, 0, x]
    return [(r + m) * 255, (g + m) * 255, (b + m) * 255]
}

/// Parse a CSS colour: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()` /
/// `rgba()` and `hsl()` / `hsla()` in comma or space syntax, and
/// `transparent`. Anything else is `null`.
export function parseColor(value: string): Rgba | null {
    const v = value.trim().toLowerCase()
    if (v === "transparent") { return { r: 0, g: 0, b: 0, a: 0 } }
    const hex = /^#([0-9a-f]{3,8})$/.exec(v)
    if (hex) {
        let digits = hex[1]
        if (digits.length === 3 || digits.length === 4) {
            digits = [...digits].map((d) => d + d).join("")
        }
        if (digits.length !== 6 && digits.length !== 8) { return null }
        const byte = (i: number) => parseInt(digits.slice(i, i + 2), 16)
        return { r: byte(0), g: byte(2), b: byte(4), a: digits.length === 8 ? byte(6) / 255 : 1 }
    }
    const fn = /^(rgba?|hsla?)\((.*)\)$/.exec(v)
    if (!fn) { return null }
    const parts = fn[2].replace(/\//g, " ").replace(/,/g, " ").trim().split(/\s+/)
    if (parts.length < 3) { return null }
    const alpha = (raw: string | undefined) => {
        if (raw === undefined) { return 1 }
        return raw.endsWith("%") ? parseFloat(raw) / 100 : parseFloat(raw)
    }
    if (fn[1].startsWith("rgb")) {
        const channel = (raw: string) =>
            clampByte(raw.endsWith("%") ? (parseFloat(raw) / 100) * 255 : parseFloat(raw))
        return { r: channel(parts[0]), g: channel(parts[1]), b: channel(parts[2]), a: alpha(parts[3]) }
    }
    const [r, g, b] = hslToRgb(parseFloat(parts[0]), parseFloat(parts[1]) / 100, parseFloat(parts[2]) / 100)
    return { r, g, b, a: alpha(parts[3]) }
}

/// `fg` over the opaque `bg`, as the browser composites it.
export function composite(fg: Rgba, bg: Rgba): Rgba {
    const a = fg.a
    return {
        r: fg.r * a + bg.r * (1 - a),
        g: fg.g * a + bg.g * (1 - a),
        b: fg.b * a + bg.b * (1 - a),
        a: 1,
    }
}

function linear(channel: number): number {
    const c = channel / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
}

/// WCAG 2.x relative luminance.
export function relativeLuminance(c: Rgba): number {
    return 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
}

/// WCAG 2.x contrast ratio, (L1 + 0.05) / (L2 + 0.05) with L1 the lighter.
export function contrastRatio(a: Rgba, b: Rgba): number {
    const la = relativeLuminance(a)
    const lb = relativeLuminance(b)
    const [hi, lo] = la >= lb ? [la, lb] : [lb, la]
    return (hi + 0.05) / (lo + 0.05)
}

/// CIELAB lightness L* (0–100), the measure behind "differ in lightness":
/// equal L* reads as equal lightness whatever the hue, which is what a reader
/// who cannot tell the hues apart is left with.
export function lightness(c: Rgba): number {
    const y = relativeLuminance(c)
    return y > 216 / 24389 ? 116 * Math.cbrt(y) - 16 : (24389 / 27) * y
}

/// The floors (`theme-system`: *Every Theme Meets the Same Legibility
/// Floors*).
export const TEXT_FLOOR = 4.5
export const EDGE_FLOOR = 3
/// How far apart, in CIELAB L*, two state colours must sit to "differ in
/// lightness": about one step of a tonal ramp — far enough to tell apart in
/// greyscale, which is what a reader who cannot tell the hues apart sees.
export const LIGHTNESS_STEP = 8

/// One measured pair: a foreground token on a background token, and the floor
/// it must reach. A translucent background is composited over `backdrop`
/// first (Mycelium's washes are rgba; every other theme writes its washes as
/// the opaque values they composite to).
export interface ContrastPair {
    fg: string
    bg: string
    floor: number
    backdrop?: string
    why: string
}

const surfacesForText = ["--bg", "--surface", "--surface-2", "--surface-3"]
const rowSurfaces: Array<[string, string]> = [
    ["--selection-fill", "the selected row"],
    ["--selection-fill-hover", "a hovered selected row"],
    ["--accent-tint", "a hovered panel row"],
    ["--ok-tint", "a completed row"],
]

/// The pairs every theme, in both schemes, must clear.
export const CONTRAST_PAIRS: readonly ContrastPair[] = [
    // Body and informational secondary text, on every plane and row state.
    ...["--text", "--text-muted"].flatMap((fg) => [
        ...surfacesForText.map((bg) => ({ fg, bg, floor: TEXT_FLOOR, why: "text on a plane" })),
        ...rowSurfaces.map(([bg, why]) => ({ fg, bg, floor: TEXT_FLOOR, backdrop: "--surface", why: `text on ${why}` })),
    ]),
    // The faint tone carries change ids and times: on the planes it sits on,
    // the hovered row (--surface-2) and the selected row.
    ...["--bg", "--surface", "--surface-2"].map((bg) => ({
        fg: "--text-faint",
        bg,
        floor: TEXT_FLOOR,
        why: "faint text on a plane",
    })),
    {
        fg: "--text-faint",
        bg: "--selection-fill",
        floor: TEXT_FLOOR,
        backdrop: "--surface",
        why: "faint text on the selected row",
    },
    // Inline code, on the reading surfaces and on its own chip.
    ...["--bg", "--surface"].map((bg) => ({ fg: "--code-fg", bg, floor: TEXT_FLOOR, why: "inline code" })),
    { fg: "--code-fg", bg: "--code-bg", floor: TEXT_FLOOR, backdrop: "--bg", why: "inline code on its chip" },
    // Links and the done ink are informational text too.
    ...["--bg", "--surface", "--surface-2"].map((bg) => ({ fg: "--accent", bg, floor: TEXT_FLOOR, why: "a link" })),
    {
        fg: "--selection-link",
        bg: "--selection-fill",
        floor: TEXT_FLOOR,
        backdrop: "--surface",
        why: "a link on the selected row",
    },
    ...["--bg", "--surface"].map((bg) => ({ fg: "--ok-strong", bg, floor: TEXT_FLOOR, why: "the done ink" })),
    // A diff's counts.
    ...["--diff-added-ink", "--diff-removed-ink"].flatMap((fg) =>
        ["--surface", "--surface-2"].map((bg) => ({ fg, bg, floor: TEXT_FLOOR, why: "a diff count" })),
    ),
    // Syntax highlighting on the code well, and on the diff tints (comments
    // are exempt on the tints).
    ...["--hl-keyword", "--hl-string", "--hl-number", "--hl-type", "--hl-title", "--hl-comment"].map((fg) => ({
        fg,
        bg: "--surface",
        floor: TEXT_FLOOR,
        why: "highlighted code on the code well",
    })),
    ...["--hl-keyword", "--hl-string", "--hl-number", "--hl-type", "--hl-title", "--text", "--text-muted"].flatMap(
        (fg) =>
            ["--diff-added-bg", "--diff-removed-bg"].map((bg) => ({
                fg,
                bg,
                floor: TEXT_FLOOR,
                why: "code on a diff tint",
            })),
    ),
    // A workspace-tinted branch chip's ink.
    ...["indigo", "blue", "teal", "green", "amber", "orange", "rose", "purple"].map((hue) => ({
        fg: `--ws-text-${hue}`,
        bg: "--surface",
        floor: TEXT_FLOOR,
        why: "a tinted branch chip",
    })),
    // The primary button's label on its fill, at rest, hovered and pressed.
    ...["--accent-strong", "--primary-fill-hover", "--primary-fill-active"].map((bg) => ({
        fg: "--on-accent",
        bg,
        floor: TEXT_FLOOR,
        why: "the primary button's label",
    })),
    // Load-bearing edges and the focus indicator.
    ...surfacesForText.map((bg) => ({
        fg: "--border-strong",
        bg,
        floor: EDGE_FLOOR,
        why: "a load-bearing edge",
    })),
    ...["--bg", "--surface"].map((bg) => ({ fg: "--accent", bg, floor: EDGE_FLOOR, why: "the focus ring" })),
]

/// The state colours that must differ in lightness as well as hue: the
/// accent, the done ink, and the error and removal inks.
export const LIGHTNESS_PAIRS: ReadonlyArray<readonly [string, string]> = [
    ["--accent", "--ok-strong"],
    ["--accent", "--warn"],
    ["--accent", "--diff-removed-ink"],
    ["--ok-strong", "--warn"],
    ["--ok-strong", "--diff-removed-ink"],
]

/// A pair's measured ratio in one resolved scheme.
export function measure(pair: ContrastPair, tokens: TokenMap, shared: TokenMap = {}): number {
    const backdrop = pair.backdrop ? tokenColor(pair.backdrop, tokens, shared) : undefined
    const bg = tokenColor(pair.bg, tokens, shared, backdrop)
    const fg = tokenColor(pair.fg, tokens, shared, bg)
    return contrastRatio(fg, bg)
}

/// A theme token as an opaque colour: resolved, parsed, and composited over
/// `backdrop` when it carries alpha. A translucent token with no backdrop, or
/// a value that is not a colour, throws, naming the token.
export function tokenColor(name: string, tokens: TokenMap, shared: TokenMap = {}, backdrop?: Rgba): Rgba {
    const raw = tokens[name] ?? shared[name]
    if (raw === undefined) { throw new Error(`${name} is not defined`) }
    const color = parseColor(resolveValue(raw, tokens, shared))
    if (!color) { throw new Error(`${name} is not a colour: ${raw}`) }
    if (color.a >= 1) { return color }
    if (!backdrop) { throw new Error(`${name} is translucent and needs a backdrop`) }
    return composite(color, backdrop)
}
