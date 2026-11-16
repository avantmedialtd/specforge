import { useCallback, useEffect, useMemo, useSyncExternalStore } from "react"
import { getAppearance, onAppearanceChanged, setAppearance } from "../api"
import {
    DARK_SCHEME_QUERY,
    applyAppearance,
    currentAppearance,
    normalizeAppearance,
    subscribeAppearance,
    type AppliedAppearance,
} from "../appearance"
import type { Appearance } from "../types"

/// Keeps this window's stamp in step with the stored appearance, the same
/// three ways `useDocumentWidth` keeps the reading width in step, once per
/// window however many components ask (`theme-system`: *The Appearance
/// Persists and Reaches Every Window*):
///
/// - **The authoritative read.** `main.tsx` painted the first frame from the
///   `localStorage` mirror; this reads the stored pair and re-stamps if they
///   disagree — which is what corrects a theme changed by another instance
///   while this window was closed (*A stale last-known value is corrected*).
/// - **`appearance-changed`.** A change made in any other window, or in a
///   browser tab the same service serves, arrives as the service's notice
///   carrying the whole pair.
/// - **The operating system's scheme.** Under System, a change to
///   `prefers-color-scheme` re-resolves the stamped scheme while running.
///
/// Reference-counted so it runs once per window: the first holder starts it
/// and the last one to unmount stops it. Each window root holds it through
/// `useAppearanceSync`, so the window follows the appearance whether or not
/// anything on screen reads it.
let holders = 0
let stopSync: (() => void) | null = null
/// Bumped by every local choice, so an authoritative read that set out before
/// the choice cannot land after it and undo what the reader just picked.
let localWrites = 0

function acquireAppearanceSync(): () => void {
    holders += 1
    if (holders === 1) { stopSync = startAppearanceSync() }
    let released = false
    return () => {
        if (released) { return }
        released = true
        holders -= 1
        if (holders === 0) {
            stopSync?.()
            stopSync = null
        }
    }
}

function startAppearanceSync(): () => void {
    let stopped = false
    const writesAtStart = localWrites
    getAppearance()
        .then((stored) => {
            if (stopped || localWrites !== writesAtStart) { return }
            applyAppearance(normalizeAppearance(stored))
        })
        .catch((err) => {
            // A failed read leaves the mirrored appearance in place, which is
            // the right fallback: the reader keeps the look they last chose
            // rather than being snapped to the default by a transport blip.
            console.warn("failed to read the appearance", err)
        })

    const unlisten = onAppearanceChanged((next) => {
        if (!stopped) { applyAppearance(normalizeAppearance(next)) }
    })

    let query: MediaQueryList | null = null
    const onSchemeChange = () => {
        const { theme, scheme } = currentAppearance()
        if (scheme === "system") { applyAppearance({ theme, scheme }) }
    }
    try {
        query = globalThis.matchMedia?.(DARK_SCHEME_QUERY) ?? null
        query?.addEventListener("change", onSchemeChange)
    } catch {
        query = null
    }

    return () => {
        stopped = true
        void unlisten.then((off) => off())
        query?.removeEventListener("change", onSchemeChange)
    }
}

/// Hold this window's appearance sync without re-rendering on a change — what
/// a window root needs (`App`, and every detached root through
/// `useDetachedWindow`).
export function useAppearanceSync(): void {
    useEffect(() => acquireAppearanceSync(), [])
}

/// The appearance in effect, re-rendering on every change to it, and an
/// optimistic `choose`.
export interface AppearanceControl extends AppliedAppearance {
    /// Stamp `next` at once, then store it. Resolves once it is stored; a write
    /// the backend rejects puts the previous appearance back and rejects in
    /// turn, so Settings can report the failure beside the control the reader
    /// operated while every window shows what is actually stored
    /// (`settings-view`: *Settings Persist by One Rule*).
    choose: (next: Appearance) => Promise<void>
}

/// The appearance in effect, for everything drawn from tokens at run time —
/// diagrams, embedded SVG, the Settings cards — which re-render when either
/// the theme or the resolved scheme changes (`theme-system`: *Token-Derived
/// Renderings Follow the Active Appearance*). Holds the window's sync too, so
/// a component that reads the appearance never depends on a root to keep it
/// current.
export function useAppearance(): AppearanceControl {
    const applied = useSyncExternalStore(subscribeAppearance, currentAppearance, currentAppearance)
    useEffect(() => acquireAppearanceSync(), [])

    const choose = useCallback(async (next: Appearance) => {
        const { theme, scheme } = currentAppearance()
        const previous: Appearance = { theme, scheme }
        localWrites += 1
        applyAppearance(next)
        try {
            await setAppearance(normalizeAppearance(next))
        } catch (err) {
            applyAppearance(previous)
            throw err
        }
    }, [])

    return useMemo(() => ({ ...applied, choose }), [applied, choose])
}
