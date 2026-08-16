import { useCallback, useEffect, useId, useRef, useState } from "react"
import { copyElement } from "../clipboard"

/// How long the "copied" confirmation stays up. Long enough to register,
/// short enough that it never reads as a permanent state change.
const CONFIRM_MS = 1400

interface CopyableIdentityProps {
    /// The identity itself — a change directory name, an archive directory
    /// name, or a file's root-relative path. Rendered verbatim.
    value: string
    /// What the value is, for the accessible name and the announcement
    /// ("change name", "file path"). Lower case; used mid-sentence.
    noun: string
}

/// An identity string that copies itself when clicked.
///
/// Click-to-copy rather than select-and-copy-yourself because the identity
/// exists to be pasted somewhere else — usually into a prompt for a coding
/// agent — so the copy is the whole point of the element, and making the user
/// perform a second gesture to complete an action they already committed to is
/// friction with no upside.
///
/// It is nonetheless still *selectable* (`user-select: all`, applied by the
/// stylesheet): the click selects the value as well as copying it, so the
/// highlight is a free, immediate confirmation of exactly what landed on the
/// clipboard — and if the clipboard write is refused, the selection is already
/// in place for the platform's own copy shortcut.
///
/// A span with `role="button"` rather than a real `<button>`: WebKit sets
/// `user-select: none` on form controls in its UA stylesheet, and the desktop
/// app runs in a WKWebView, so a `<button>` would put the selection behaviour
/// on exactly the host that cannot be checked from here. The span carries the
/// button's semantics explicitly instead — a real tab stop, Enter/Space
/// activation, and an accessible name — because this lives in the detail pane,
/// not the tree, so it is free to take a tab stop without disturbing the
/// tree's roving-focus, single-Tab-stop model.
export function CopyableIdentity({ value, noun }: CopyableIdentityProps) {
    const ref = useRef<HTMLSpanElement>(null)
    const [state, setState] = useState<"idle" | "copied" | "failed">("idle")
    const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
    const statusId = useId()

    // Any pending confirmation belongs to the value that was copied; if the
    // pane moves to another artifact the old confirmation must not outlive it.
    useEffect(() => {
        setState("idle")
        if (timer.current) clearTimeout(timer.current)
    }, [value])

    useEffect(
        () => () => {
            if (timer.current) clearTimeout(timer.current)
        },
        [],
    )

    const copy = useCallback(() => {
        const el = ref.current
        if (!el) return
        void copyElement(el).then((ok) => {
            setState(ok ? "copied" : "failed")
            if (timer.current) clearTimeout(timer.current)
            timer.current = setTimeout(() => setState("idle"), CONFIRM_MS)
        })
    }, [])

    return (
        <>
            <span
                ref={ref}
                className={`identity-name${state === "copied" ? " identity-name--copied" : ""}${
                    state === "failed" ? " identity-name--failed" : ""
                }`}
                role="button"
                tabIndex={0}
                aria-label={`Copy ${noun} ${value}`}
                aria-describedby={statusId}
                title={`Click to copy this ${noun}`}
                onClick={copy}
                onKeyDown={(e) => {
                    // Enter and Space are the button activation keys. Space is
                    // preventDefault'ed so it cannot scroll the pane out from
                    // under the reader on the way to copying.
                    if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault()
                        copy()
                    }
                }}
            >
                {value}
            </span>
            {/* Announced, never laid out: a visible "Copied" label would change
                the element's width, and the identity shares a flex row with the
                branch chip, so the row would jump on every copy. The visible
                confirmation is the colour flash plus the selection highlight,
                neither of which reflows anything. */}
            <span id={statusId} className="sr-only" aria-live="polite">
                {state === "copied"
                    ? `Copied ${noun} ${value}`
                    : state === "failed"
                      ? `Could not copy. The ${noun} is selected — use your copy shortcut.`
                      : ""}
            </span>
        </>
    )
}
