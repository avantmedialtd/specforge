import { useRef } from "react"

/// A ref that always holds the value of the latest render: what a callback
/// kept across renders (a cached slot's handler, a scheduler's load) reads at
/// call time, so it never acts on a stale closure while its own identity stays
/// fixed.
///
/// Written during render, the pattern `WorkspaceTree`'s `onSelectRef` and
/// `DiffView`'s mirrors already use. So the ref holds the value of the latest
/// render, committed or not: a render React throws away leaves its value
/// behind until the next render writes over it. Callers read it from event
/// handlers and timers, where the value of a render that is about to be
/// retried is as good as the committed one; nothing here may depend on it
/// matching what is on screen.
export function useLatest<T>(value: T): { readonly current: T } {
    const ref = useRef(value)
    ref.current = value
    return ref
}
