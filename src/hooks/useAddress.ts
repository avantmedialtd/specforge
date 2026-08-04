import { useCallback, useMemo, useSyncExternalStore } from "react"
import type { Address, Unresolvable } from "../routing/address"
import { encodeAddress } from "../routing/codec"
import { createAddressSnapshot, createHistory } from "../routing/history"

export interface UseAddressResult {
    /// The currently decoded address — `Unresolvable` when the current path
    /// does not match the Address grammar at all (a hand-typed/garbled URL;
    /// distinct from a well-formed address that fails to *resolve* against
    /// the registry, which `resolve.ts` reports as "not found").
    address: Address | Unresolvable
    /// Navigate to `address`. A no-op when it encodes to the path already
    /// current (covers, among others, re-selecting a section/task row within
    /// the artifact already showing — see *History Entry Discipline*'s
    /// "scrolling creates no entry"). `replace: true` swaps the current
    /// entry instead of pushing a new one (canonicalisation).
    navigate: (address: Address, options?: { replace?: boolean }) => void
    /// Move one entry back in the owned history adapter — used by the
    /// desktop-only back/forward keyboard handler (*Desktop Back and Forward
    /// Gestures*). A no-op at the start of history.
    back: () => void
    /// Move one entry forward. A no-op at the end of history.
    forward: () => void
}

/// Owns the current Address by subscribing to the host-appropriate history
/// adapter (`view-routing`: *Host-Detected History Adapter*) and decoding its
/// current path. One adapter instance per mount — the desktop shell only
/// ever mounts `App` once, and tests construct their own history directly
/// rather than through this hook.
export function useAddress(): UseAddressResult {
    const history = useMemo(() => createHistory(), [])
    // `createAddressSnapshot` caches the decoded Address against the path it
    // came from, so `getSnapshot` is Object.is-stable while the path is
    // unchanged — `useSyncExternalStore`'s hard requirement (a plain
    // `() => decodeAddress(history.current())` allocates a fresh object
    // every call and infinite-loops React trying to settle it; see
    // `createAddressSnapshot`'s own doc comment in `routing/history.ts`).
    const getSnapshot = useMemo(() => createAddressSnapshot(history), [history])

    const address = useSyncExternalStore(history.subscribe, getSnapshot)

    const navigate = useCallback(
        (next: Address, options?: { replace?: boolean }) => {
            const path = encodeAddress(next)
            if (options?.replace) history.replace(path)
            else history.push(path)
        },
        [history],
    )

    return {
        address,
        navigate,
        back: history.back,
        forward: history.forward,
    }
}
