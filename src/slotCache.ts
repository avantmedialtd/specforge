// A per-key cache of built values, kept while the inputs they were built
// from are unchanged (`diff-view`: *Host Slot Updates Re-render Only Their
// File*; `pull-request-viewer`: *Review Marks Re-render Only the Marked File*;
// design D5).
//
// The pull-request view builds each file's slots through one of these, keyed
// by the file and signed by a flat tuple of that file's own inputs
// (`fileSlotSignature` in `pullRequestView.ts`). An unchanged signature hands
// back the very value built before, so `DiffView` sees the same slots and the
// file's section skips its render.

/// A value's inputs, compared element by element with `Object.is`.
export type Signature = readonly unknown[]

export interface SlotCache<V> {
    /// The value cached for `key` when its stored signature equals
    /// `signature` (same length, `Object.is` per element); otherwise the
    /// value `build` returns, stored under `signature`.
    get: (key: string, signature: Signature, build: () => V) => V
    /// Drops every key not in `keys`: the files that are gone.
    retain: (keys: Iterable<string>) => void
}

export function sameSignature(a: Signature, b: Signature): boolean {
    if (a.length !== b.length) return false
    for (let index = 0; index < a.length; index += 1) {
        if (!Object.is(a[index], b[index])) return false
    }
    return true
}

export function createSlotCache<V>(): SlotCache<V> {
    const entries = new Map<string, { signature: Signature; value: V }>()
    return {
        get(key, signature, build) {
            const entry = entries.get(key)
            if (entry && sameSignature(entry.signature, signature)) return entry.value
            const value = build()
            entries.set(key, { signature, value })
            return value
        },
        retain(keys) {
            const kept = new Set(keys)
            for (const key of [...entries.keys()]) {
                if (!kept.has(key)) entries.delete(key)
            }
        },
    }
}

/// `previous` when it holds the same keys as `next`, in the same order, each
/// with an `Object.is`-equal value; otherwise `next`. What lets `DiffView`
/// keep its per-file map, and with it every memo and effect keyed on that map,
/// across a host render that changed no file's slots.
export function reuseMap<K, V>(
    previous: ReadonlyMap<K, V> | null,
    next: ReadonlyMap<K, V>,
): ReadonlyMap<K, V> {
    if (previous === null || previous.size !== next.size) return next
    const before = previous.entries()
    for (const [key, value] of next) {
        const prior = before.next()
        if (prior.done || !Object.is(prior.value[0], key) || !Object.is(prior.value[1], value)) {
            return next
        }
    }
    return previous
}
