/// The appearance a window paints in: the stored theme and scheme, how
/// "System" resolves, the first-paint mirror, and the stamp on `<html>`
/// (`theme-system`: *The Scheme Is a Selectable Preference*, *The Appearance
/// Is In Effect On the First Paint*).
///
/// CSS never sees "system". The stamp carries the theme and the RESOLVED
/// scheme as `data-theme` / `data-scheme`, which is all the stylesheets key
/// on (design D2), and the root's `color-scheme` follows the resolved scheme
/// so native scrollbars and form controls do too.
///
/// The first stamp is not made here. A hand copy of `readMirroredAppearance`,
/// `resolveScheme` and the stamp in `applyAppearance` runs inline in the
/// `<head>` of `index.html`, before the bundle exists, and
/// `src/firstPaintStamp.test.ts` holds it equal to this module: change the
/// storage key, the ids or the default pair and that copy changes with them
/// (design D14).
///
/// What is stamped right now is also the one live value the rest of the page
/// reads: `currentAppearance()` / `subscribeAppearance()` are a tiny external
/// store, so every consumer — a diagram, an embedded SVG, the Settings cards
/// — redraws from the same stamp the stylesheets just changed under, in the
/// same task. Kept free of React and of the transport, so it is unit-tested
/// without a DOM: every DOM touch goes through a root element that tests can
/// fake, and every ambient read (`localStorage`, `matchMedia`,
/// `getComputedStyle`) is guarded, because this runs before React mounts, on
/// the path that paints the first frame.

import type { Appearance, ColorScheme, Theme } from "./types"

/// The scheme a stylesheet sees.
export type ResolvedScheme = "light" | "dark"

/// The themes, in the order Settings offers them (`settings-view`: *The
/// Appearance Group Offers Theme and Scheme*).
export const THEMES: readonly Theme[] = ["forge", "blueprint", "folio", "mycelium"]

/// The scheme choices, in the order Settings offers them.
export const SCHEMES: readonly ColorScheme[] = ["system", "light", "dark"]

/// What an installation with nothing stored renders in, and what any value
/// neither side recognises folds to — the backend applies the same rule when
/// it reads its settings file.
export const DEFAULT_APPEARANCE: Appearance = { theme: "forge", scheme: "system" }

/// Fold any value to a theme. `hasOwnProperty`-style membership, never `in`,
/// so an inherited name such as `"constructor"` is not a theme.
export function normalizeTheme(value: unknown): Theme {
    return typeof value === "string" && (THEMES as readonly string[]).includes(value)
        ? (value as Theme)
        : DEFAULT_APPEARANCE.theme
}

/// Fold any value to a scheme choice.
export function normalizeScheme(value: unknown): ColorScheme {
    return typeof value === "string" && (SCHEMES as readonly string[]).includes(value)
        ? (value as ColorScheme)
        : DEFAULT_APPEARANCE.scheme
}

/// Fold any value to an appearance, half by half: an unknown half takes its
/// default while the other holds, and anything that is not an object at all —
/// an unparseable event frame, a hand-edited mirror — is the default pair.
export function normalizeAppearance(value: unknown): Appearance {
    if (typeof value !== "object" || value === null) { return { ...DEFAULT_APPEARANCE } }
    const halves = value as { theme?: unknown; scheme?: unknown }
    return { theme: normalizeTheme(halves.theme), scheme: normalizeScheme(halves.scheme) }
}

/// The light or dark scheme a choice renders in, given what the operating
/// system reports. Under System it is the operating system's; an explicit
/// choice overrides it.
export function resolveScheme(scheme: ColorScheme, prefersDark: boolean): ResolvedScheme {
    if (scheme === "light" || scheme === "dark") { return scheme }
    return prefersDark ? "dark" : "light"
}

/// The media query System follows.
export const DARK_SCHEME_QUERY = "(prefers-color-scheme: dark)"

/// Whether the operating system reports dark, or `false` where there is no
/// `matchMedia` to ask (a test runtime, a host that never answers).
export function prefersDarkScheme(): boolean {
    try {
        return globalThis.matchMedia?.(DARK_SCHEME_QUERY).matches ?? false
    } catch {
        return false
    }
}

/// Where the first-paint mirror lives.
export const APPEARANCE_STORAGE_KEY = "specforge:appearance"

/// The subset of `Storage` the mirror uses, so tests can pass a fake.
export interface AppearanceStore {
    getItem(key: string): string | null
    setItem(key: string, value: string): void
}

/// The ambient store, or `null` where there isn't one. Reading
/// `globalThis.localStorage` can itself throw — a browser set to block site
/// data does exactly that — so the access is guarded, not just the call.
function ambientStore(): AppearanceStore | null {
    try {
        return globalThis.localStorage ?? null
    } catch {
        return null
    }
}

/// Read the mirrored appearance. Never throws and never returns anything but
/// an appearance: an absent, unreadable or unparseable mirror is the default
/// pair (`theme-system`: *A window with no applied appearance still renders*).
export function readMirroredAppearance(
    store: AppearanceStore | null = ambientStore(),
): Appearance {
    try {
        const raw = store?.getItem(APPEARANCE_STORAGE_KEY)
        return raw ? normalizeAppearance(JSON.parse(raw)) : { ...DEFAULT_APPEARANCE }
    } catch {
        return { ...DEFAULT_APPEARANCE }
    }
}

/// Mirror the appearance for the next cold start. Best-effort by design: the
/// authoritative value is in the application settings, and this store failing
/// costs one frame in the default on some later launch, nothing more.
export function writeMirroredAppearance(
    appearance: Appearance,
    store: AppearanceStore | null = ambientStore(),
): void {
    try {
        store?.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(normalizeAppearance(appearance)))
    } catch {
        // Deliberately swallowed; see above.
    }
}

/// The appearance in effect: the stored pair as last applied, and the scheme
/// it resolved to.
export interface AppliedAppearance {
    theme: Theme
    scheme: ColorScheme
    resolvedScheme: ResolvedScheme
}

/// The part of an element the stamp writes. A real `HTMLElement` is one; so is
/// a test's fake.
export interface StampTarget {
    setAttribute(name: string, value: string): void
    style: { colorScheme?: string }
}

/// What `applyAppearance` reads and writes besides the stamp, each defaulting
/// to the ambient one.
export interface ApplyOptions {
    root?: StampTarget
    prefersDark?: boolean
    store?: AppearanceStore | null
}

let applied: AppliedAppearance = {
    ...DEFAULT_APPEARANCE,
    resolvedScheme: "light",
}
const listeners = new Set<() => void>()

/// The appearance stamped now. The same object until the next stamp that
/// changes something, so it is a valid `useSyncExternalStore` snapshot.
export function currentAppearance(): AppliedAppearance {
    return applied
}

/// Be told after every stamp that changes the appearance. Returns the
/// unsubscribe function.
export function subscribeAppearance(listener: () => void): () => void {
    listeners.add(listener)
    return () => {
        listeners.delete(listener)
    }
}

function ambientRoot(): StampTarget | null {
    return typeof document === "undefined" ? null : document.documentElement
}

/// Stamp `appearance` on the root, set its `color-scheme`, keep the browser's
/// theme colour in step, and mirror it for the next cold start — then tell
/// every subscriber. The two attributes are written together, in one task, so
/// no frame ever shows a new theme in an old scheme. Returns what is now in
/// effect.
export function applyAppearance(
    appearance: Appearance,
    options: ApplyOptions = {},
): AppliedAppearance {
    const next = normalizeAppearance(appearance)
    const resolvedScheme = resolveScheme(next.scheme, options.prefersDark ?? prefersDarkScheme())
    const root = options.root ?? ambientRoot()
    if (root) {
        root.setAttribute("data-theme", next.theme)
        root.setAttribute("data-scheme", resolvedScheme)
        root.style.colorScheme = resolvedScheme
    }
    writeMirroredAppearance(next, options.store)
    if (root && root === ambientRoot()) { syncThemeColor() }

    if (
        applied.theme !== next.theme ||
        applied.scheme !== next.scheme ||
        applied.resolvedScheme !== resolvedScheme
    ) {
        applied = { theme: next.theme, scheme: next.scheme, resolvedScheme }
        for (const listener of [...listeners]) { listener() }
    }
    return applied
}

/// Point the browser's own chrome at the active theme's background in the
/// resolved scheme (`theme-system`: *The browser chrome colour follows the
/// theme*), read from `--bg` on the root so the colour stays the stylesheet's
/// and is never a fifth copy of a palette. Guarded throughout: a host with no
/// computed style or no meta tag simply keeps what it had.
function syncThemeColor(): void {
    try {
        const bg = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim()
        if (!bg) { return }
        let meta = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]')
        if (!meta) {
            meta = document.createElement("meta")
            meta.name = "theme-color"
            document.head.appendChild(meta)
        }
        meta.content = bg
    } catch {
        // No style engine or no head to write to; nothing to keep in step.
    }
}
