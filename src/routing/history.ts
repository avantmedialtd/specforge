// Two History implementations behind one interface, selected by host
// (`view-routing`: *Host-Detected History Adapter*). Both operate on plain
// path strings — Address encode/decode happens one layer up, in
// `useAddress.ts` — so the adapters themselves have no notion of what a path
// means. `createAddressSnapshot` at the bottom of this file is the one
// exception: it bridges a `History` to the `useSyncExternalStore` contract
// `useAddress.ts` needs, which requires it to know about `Address`.

import { isTauri } from "../api"
import type { Address, Unresolvable } from "./address"
import { decodeAddress } from "./codec"

export interface History {
    /// The current path.
    current: () => string
    /// Navigate to `path`, creating a new entry.
    push: (path: string) => void
    /// Replace the current entry with `path` — no new entry (used when
    /// canonicalising an address rather than navigating to a new view; see
    /// *History Entry Discipline*).
    replace: (path: string) => void
    /// Subscribe to every change of `current()` — from `push`/`replace`,
    /// `back`/`forward`, or (browser adapter) the user's own gestures.
    /// No-arg callback, mirroring the external-store pattern already used by
    /// `WorkspaceTree`'s selection store — callers re-read `current()`.
    subscribe: (callback: () => void) => () => void
    /// Move one entry back, notifying subscribers. A no-op at the start of
    /// history.
    back: () => void
    /// Move one entry forward, notifying subscribers. A no-op at the end of
    /// history.
    forward: () => void
}

/// Backed by the browser's own session history (`pushState`/`popstate`), so
/// the address is reflected in `window.location`, can be bookmarked, and
/// survives a reload (*The served UI reflects the address in the browser
/// location*). `push`/`replace` notify subscribers directly —
/// `pushState`/`replaceState` never fire `popstate` on their own; `back`/
/// `forward` rely on the `popstate` event the browser fires for them, so a
/// real user gesture and a programmatic one notify the same way.
export function createBrowserHistory(): History {
    const listeners = new Set<() => void>()
    const notify = () => listeners.forEach((cb) => cb())

    window.addEventListener("popstate", notify)

    return {
        current: () => window.location.pathname,
        push: (path) => {
            if (path === window.location.pathname) return
            window.history.pushState(null, "", path)
            notify()
        },
        replace: (path) => {
            if (path === window.location.pathname) return
            window.history.replaceState(null, "", path)
            notify()
        },
        subscribe: (cb) => {
            listeners.add(cb)
            return () => listeners.delete(cb)
        },
        back: () => window.history.back(),
        forward: () => window.history.forward(),
    }
}

/// An entry array plus an index — the desktop shell's adapter, and what
/// makes the whole navigation layer testable without `window.history`.
/// Starts with a single entry (`/` by default — the home surface, matching a
/// fresh cold start).
export function createMemoryHistory(initial = "/"): History {
    let entries = [initial]
    let index = 0
    const listeners = new Set<() => void>()
    const notify = () => listeners.forEach((cb) => cb())

    return {
        current: () => entries[index]!,
        push: (path) => {
            if (path === entries[index]) return
            // Drop any forward ("redo") branch — pushing after a back
            // discards it, matching standard browser-history semantics.
            entries = [...entries.slice(0, index + 1), path]
            index = entries.length - 1
            notify()
        },
        replace: (path) => {
            if (path === entries[index]) return
            entries = entries.slice()
            entries[index] = path
            notify()
        },
        subscribe: (cb) => {
            listeners.add(cb)
            return () => listeners.delete(cb)
        },
        back: () => {
            if (index === 0) return
            index -= 1
            notify()
        },
        forward: () => {
            if (index === entries.length - 1) return
            index += 1
            notify()
        },
    }
}

/// The adapter for the current host: the in-memory adapter inside the
/// SpecForge desktop shell (which loads from the Tauri asset protocol, not
/// expected to fall back to the app shell for an unknown path — see
/// design.md's *Rejected: pushState in the Tauri webview too*), the browser
/// adapter everywhere else (the served web UI).
export function createHistory(): History {
    return isTauri() ? createMemoryHistory() : createBrowserHistory()
}

/// A `useSyncExternalStore`-safe snapshot getter for `history`.
///
/// `useSyncExternalStore` compares consecutive `getSnapshot()` results with
/// `Object.is` to decide whether the store changed; a getter that allocates
/// a fresh value on every call (e.g. `() => decodeAddress(history.current())`
/// on its own — `decodeAddress` always returns a new object literal) looks
/// like a perpetually-changing store, and React infinite-loops trying to
/// settle it (error #185, "The result of getSnapshot should be cached to
/// avoid an infinite loop"). This closure caches the last decoded `Address`
/// alongside the path it came from, so repeated calls with no intervening
/// navigation return the exact same (`Object.is`-equal) reference, and only
/// decode again once `history.current()` actually differs. See
/// `useAddress.ts`, the sole caller, and `history.test.ts`'s
/// `createAddressSnapshot` suite for the invariant this exists to guarantee.
export function createAddressSnapshot(history: History): () => Address | Unresolvable {
    let lastPath = history.current()
    let lastAddress: Address | Unresolvable = decodeAddress(lastPath)
    return () => {
        const path = history.current()
        if (path !== lastPath) {
            lastPath = path
            lastAddress = decodeAddress(path)
        }
        return lastAddress
    }
}
