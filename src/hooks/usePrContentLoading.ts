import { useCallback, useEffect, useRef, useState } from "react"
import { getPrContentLoading, onPrContentLoadingChanged, setPrContentLoading } from "../api"
import type { PrContentLoading, PrContentPreference } from "../types"

export function normalizePrContentLoading(value: unknown): PrContentLoading {
    const pair = value as Partial<PrContentLoading> | null | undefined
    return {
        autoShowImages: pair?.autoShowImages === true,
        autoLoadLargeDiffs: pair?.autoLoadLargeDiffs === true,
    }
}

/// No localStorage authority: null means that no automatic read may start.
/// Subscribe before reading, and fence reads/save replies against newer notices.
export function usePrContentLoading() {
    const [preferences, setPreferences] = useState<PrContentLoading | null>(null)
    const [error, setError] = useState<string | null>(null)
    const [saveErrors, setSaveErrors] = useState<Partial<Record<PrContentPreference, string>>>({})
    const [saving, setSaving] = useState<Partial<Record<PrContentPreference, boolean>>>({})
    const revision = useRef(0)
    const alive = useRef(false)
    const pending = useRef(new Set<PrContentPreference>())

    useEffect(() => {
        alive.current = true
        let stopped = false
        const unlisten = onPrContentLoadingChanged(pair => {
            if (stopped) { return }
            revision.current += 1
            setPreferences(normalizePrContentLoading(pair))
            setError(null)
        })
        void unlisten.then(async () => {
            if (stopped) { return }
            const atStart = revision.current
            const pair = await getPrContentLoading()
            if (stopped || atStart !== revision.current) { return }
            setPreferences(normalizePrContentLoading(pair))
            setError(null)
        }).catch(() => {
            if (!stopped && revision.current === 0) { setError("Could not load PR loading preferences.") }
        })
        return () => {
            stopped = true
            alive.current = false
            void unlisten.then(off => off()).catch(() => undefined)
        }
    }, [])

    const choose = useCallback(async (preference: PrContentPreference, enabled: boolean) => {
        if (pending.current.has(preference)) { return }
        pending.current.add(preference)
        const atStart = revision.current
        setSaving(p => ({ ...p, [preference]: true }))
        setSaveErrors(p => ({ ...p, [preference]: undefined }))
        try {
            const pair = await setPrContentLoading(preference, enabled)
            if (alive.current && atStart === revision.current) {
                revision.current += 1
                setPreferences(normalizePrContentLoading(pair))
            }
        } catch {
            // Never publish optimistic authority. A failed write leaves the
            // stored pair (including another client's newer notice) intact.
            if (alive.current) { setSaveErrors(p => ({ ...p, [preference]: "Couldn't save this setting." })) }
        } finally {
            pending.current.delete(preference)
            if (alive.current) { setSaving(p => ({ ...p, [preference]: false })) }
        }
    }, [])
    return { preferences, error, saving, saveErrors, choose }
}
