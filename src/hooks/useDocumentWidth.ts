import { useCallback, useEffect, useRef, useState } from "react"
import { getDocumentWidth, onDocumentWidthChanged, setDocumentWidth } from "../api"
import {
    readMirroredDocumentWidth,
    writeMirroredDocumentWidth,
} from "../docWidth"
import type { DocumentWidth } from "../types"

/// Stamp the rung onto `<body>` and mirror it for the next cold start.
///
/// The stamp is what the stylesheet reads (`body[data-doc-width="…"]`); the
/// mirror is what `main.tsx` reads before React mounts, so the next launch
/// paints at this width rather than reflowing into it. The two are written
/// together because a stamp without a mirror would be forgotten on restart and
/// a mirror without a stamp would not be visible until one.
export function applyDocumentWidth(width: DocumentWidth): void {
    document.body.dataset.docWidth = width
    writeMirroredDocumentWidth(width)
}

/// The reading width, and a setter that persists it.
///
/// Initial state comes from the mirror rather than from an awaited fetch: the
/// bootstrap has already stamped that value, so starting from it means the
/// hook's first render agrees with what is on screen. The authoritative value
/// is fetched immediately after and reconciled — that path matters when a
/// second instance of the application changed the setting since this window
/// last ran, which is exactly when the mirror is stale.
///
/// May be used from more than one component at once. Each instance keeps its
/// own listener, and they converge because every change is announced.
///
/// The setter resolves once the width is persisted. A write the backend
/// rejects puts the previous width back and rejects in turn, so Settings can
/// report the failure on the picker while the picker shows what is actually
/// stored (`settings-view`: *Settings Persist by One Rule*).
export function useDocumentWidth(): [DocumentWidth, (width: DocumentWidth) => Promise<void>] {
    const [width, setWidth] = useState<DocumentWidth>(readMirroredDocumentWidth)
    // The width the setter reverts to — read through a ref because the setter
    // is created once and would otherwise see only the first render's value.
    const widthRef = useRef(width)
    widthRef.current = width

    // Reconcile against the authoritative store.
    useEffect(() => {
        let cancelled = false
        getDocumentWidth()
            .then((authoritative) => {
                if (cancelled) return
                setWidth(authoritative)
                applyDocumentWidth(authoritative)
            })
            .catch((err) => {
                // A failed read leaves the mirrored rung in place, which is the
                // right fallback: the reader keeps the width they last chose
                // rather than being snapped to the default by a transport blip.
                console.warn("failed to read document width", err)
            })
        return () => {
            cancelled = true
        }
    }, [])

    // Adopt changes made elsewhere — another window, or the browser skin
    // against the same service. This is the half the mirror cannot do: a
    // reader window already open would otherwise keep the width it launched
    // with until it was reopened.
    useEffect(() => {
        const unlisten = onDocumentWidthChanged((next) => {
            setWidth(next)
            applyDocumentWidth(next)
        })
        return () => {
            void unlisten.then((off) => off())
        }
    }, [])

    const choose = useCallback(async (next: DocumentWidth) => {
        // Applied before the round trip so the picker feels immediate. The
        // backend's event will arrive shortly after and set the same value.
        const previous = widthRef.current
        setWidth(next)
        applyDocumentWidth(next)
        try {
            await setDocumentWidth(next)
        } catch (err) {
            setWidth(previous)
            applyDocumentWidth(previous)
            throw err
        }
    }, [])

    return [width, choose]
}
