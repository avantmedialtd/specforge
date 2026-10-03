import { useEffect, useState } from "react"
import { prettifyError } from "../errors"

/// One on/off setting a Settings group reads and writes itself: its stored
/// value (`null` until loaded), a flip that applies at once and persists, and
/// the message to show on the row when that write fails.
///
/// The flip is optimistic and a failure puts the stored value back, so the
/// switch never goes on showing a state that was not saved — and the failure
/// is reported on the row rather than to `console.warn`, which nobody using
/// the desktop app can see (`settings-view`: *Settings Persist by One Rule*).
///
/// `fallback` is what the switch shows when the stored value cannot be read
/// at all, so it stays operable rather than spinning forever.
export function useSettingSwitch(
    load: () => Promise<boolean>,
    save: (next: boolean) => Promise<void>,
    fallback: boolean,
): { value: boolean | null; flip: (next: boolean) => Promise<void>; error: string | null } {
    const [value, setValue] = useState<boolean | null>(null)
    const [error, setError] = useState<string | null>(null)

    useEffect(() => {
        let cancelled = false
        load()
            .then((stored) => {
                if (!cancelled) setValue(stored)
            })
            .catch(() => {
                if (!cancelled) setValue(fallback)
            })
        return () => {
            cancelled = true
        }
        // `load` and `fallback` are fixed for a row's lifetime.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [])

    const flip = async (next: boolean) => {
        if (value === null) return
        const previous = value
        setValue(next)
        setError(null)
        try {
            await save(next)
        } catch (err) {
            setValue(previous)
            setError(`Couldn't save this setting — ${prettifyError(err)}`)
        }
    }

    return { value, flip, error }
}
