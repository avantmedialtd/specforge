import { useCallback, useEffect, useRef, useState } from "react"
import {
    getCommitHistoryEnabled,
    onCommitHistoryEnabledChanged,
    setCommitHistoryEnabled,
} from "../api"
import { readMirroredCommitHistory, writeMirroredCommitHistory } from "../commitHistory"

/// The Commit history switch, and a setter that persists it (`commit-graph`:
/// *Commit History Can Be Turned Off*).
///
/// `useDocumentWidth`'s shape. Initial state comes from the mirror rather than
/// an awaited fetch, so the first frame already knows whether the main window
/// has a rail — a reader who turned history off never sees it painted and then
/// dropped. The authoritative value is fetched immediately after and
/// reconciled; that path matters when another instance of the application
/// changed the switch since this surface last ran, which is exactly when the
/// mirror is stale.
///
/// The setter resolves once the switch is persisted. A write the backend
/// rejects puts the previous value back and rejects in turn, so Settings can
/// report the failure on the switch while it shows what is actually stored
/// (`settings-view`: *Settings Persist by One Rule*).
export function useCommitHistoryEnabled(): [boolean, (enabled: boolean) => Promise<void>] {
    const [enabled, setEnabled] = useState<boolean>(() => readMirroredCommitHistory())
    // The value the setter reverts to — read through a ref because the setter
    // is created once and would otherwise see only the first render's value.
    const enabledRef = useRef(enabled)
    enabledRef.current = enabled

    // Reconcile against the authoritative store.
    useEffect(() => {
        let cancelled = false
        getCommitHistoryEnabled()
            .then((authoritative) => {
                if (cancelled) return
                setEnabled(authoritative)
                writeMirroredCommitHistory(authoritative)
            })
            .catch((err) => {
                // A failed read keeps the mirrored value: the reader keeps the
                // rail they last chose rather than having it flipped by a
                // transport blip.
                console.warn("failed to read the commit history switch", err)
            })
        return () => {
            cancelled = true
        }
    }, [])

    // Adopt changes made elsewhere — another window, or a browser skin against
    // the same service. An unparseable SSE frame arrives as `undefined`; only a
    // real boolean may change the layout.
    useEffect(() => {
        const unlisten = onCommitHistoryEnabledChanged((next) => {
            if (typeof next !== "boolean") return
            setEnabled(next)
            writeMirroredCommitHistory(next)
        })
        return () => {
            void unlisten.then((off) => off())
        }
    }, [])

    const choose = useCallback(async (next: boolean) => {
        // Applied before the round trip so the switch feels immediate. The
        // backend's event arrives shortly after carrying the same value.
        const previous = enabledRef.current
        setEnabled(next)
        writeMirroredCommitHistory(next)
        try {
            await setCommitHistoryEnabled(next)
        } catch (err) {
            setEnabled(previous)
            writeMirroredCommitHistory(previous)
            throw err
        }
    }, [])

    return [enabled, choose]
}
