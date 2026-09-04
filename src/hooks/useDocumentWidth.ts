import { useCallback, useEffect, useState } from "react"
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
export function useDocumentWidth(): [DocumentWidth, (width: DocumentWidth) => void] {
    const [width, setWidth] = useState<DocumentWidth>(readMirroredDocumentWidth)

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

    const choose = useCallback((next: DocumentWidth) => {
        // Applied before the round trip so the picker feels immediate. The
        // backend's event will arrive shortly after and set the same value.
        setWidth(next)
        applyDocumentWidth(next)
        setDocumentWidth(next).catch((err) => {
            console.warn("failed to persist document width", err)
        })
    }, [])

    return [width, choose]
}
