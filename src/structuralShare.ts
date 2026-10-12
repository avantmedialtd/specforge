// Structural sharing for IPC replies (`spec-browser`: *Unchanged Workspace
// Views Keep Their Rows*; `pull-request-worktree-links`: *An Unchanged Links
// Snapshot Is Not Re-applied*; design D3).
//
// Every command reply arrives as brand-new objects, even when nothing in it
// changed, and a new object defeats every `memo` and `useMemo` keyed on it.
// `shareStructure` hands back the previous value for each subtree that is
// deeply equal to its counterpart in the reply, so a reply that changes
// nothing is the previous value itself (and `setState` bails out), and a reply
// that changes one repository keeps every other repository's identity.

function isPlainObject(value: unknown): value is Record<string, unknown> {
    if (value === null || typeof value !== "object" || Array.isArray(value)) return false
    const proto: unknown = Object.getPrototypeOf(value)
    return proto === Object.prototype || proto === null
}

/// `next`, with every subtree deeply equal to its counterpart in `previous`
/// replaced by that counterpart; `previous` itself when the whole value is
/// equal.
///
/// Walks primitives, arrays and plain objects, which is everything Tauri and
/// the web transport return. Primitives compare by `Object.is`, so `NaN`
/// equals `NaN` and `-0` differs from `0`. Arrays are equal only with the same
/// length and pairwise-equal items, so order matters. Objects are equal only
/// with the same own enumerable keys and equal values, so a key that appears
/// or disappears is a change, and an `undefined` value differs from a missing
/// key. Anything else (a class instance, a function) is kept only when it is
/// the very same value. Each rule can only cause an extra render, never a
/// missed one.
export function shareStructure<T>(previous: T, next: T): T {
    return share(previous, next) as T
}

function share(previous: unknown, next: unknown): unknown {
    if (Object.is(previous, next)) return previous
    if (Array.isArray(previous) && Array.isArray(next)) return shareArray(previous, next)
    if (isPlainObject(previous) && isPlainObject(next)) return shareObject(previous, next)
    return next
}

/// `previous` when it equals `next` item for item, else a new array whose
/// equal items are the previous ones. Items are matched by position, so an
/// item inserted or removed changes every position after it: those items are
/// compared with their new neighbours and, unless equal to them, kept as the
/// reply has them. Nothing is copied until the first difference.
function shareArray(previous: unknown[], next: unknown[]): unknown[] {
    let out: unknown[] | null = previous.length === next.length ? null : []
    for (let index = 0; index < next.length; index += 1) {
        const kept = index < previous.length ? share(previous[index], next[index]) : next[index]
        if (out === null && !Object.is(kept, previous[index])) out = previous.slice(0, index)
        out?.push(kept)
    }
    return out ?? previous
}

/// `previous` when it equals `next` key for key, else a new object whose
/// equal values are the previous ones. Nothing is copied until the first
/// difference.
function shareObject(
    previous: Record<string, unknown>,
    next: Record<string, unknown>,
): Record<string, unknown> {
    const nextKeys = Object.keys(next)
    let out: Record<string, unknown> | null =
        nextKeys.length === Object.keys(previous).length ? null : {}
    for (let index = 0; index < nextKeys.length; index += 1) {
        const key = nextKeys[index]!
        const had = Object.prototype.hasOwnProperty.call(previous, key)
        const kept = had ? share(previous[key], next[key]) : next[key]
        if (out === null && (!had || !Object.is(kept, previous[key]))) {
            // The keys before this one were all equal to the previous ones.
            out = {}
            for (const before of nextKeys.slice(0, index)) out[before] = previous[before]
        }
        if (out !== null) out[key] = kept
    }
    return out ?? previous
}
