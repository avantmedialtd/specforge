import { useEffect, useRef, useState } from "react"
import { commitDecision, type Parsed } from "../../settingsFields"
import { prettifyError } from "../errors"

interface CommittedFieldProps<T> {
    id: string
    /// The stored value. The field shows it whenever no edit is in progress,
    /// and returns to it when an edit is abandoned or its write fails.
    stored: T
    format: (value: T) => string
    parse: (raw: string) => Parsed<T>
    /// How a parsed draft is compared with `stored` — `Object.is` unless the
    /// value is a list.
    equals?: (a: T, b: T) => boolean
    /// Persists a changed, valid value. Throwing reports the failure on the
    /// field and puts the stored value back.
    onCommit: (value: T) => Promise<void>
    placeholder?: string
    numeric?: boolean
    disabled?: boolean
    /// Only when no row title labels the field.
    ariaLabel?: string
}

/// A free-text or numeric field that holds one setting's value and persists
/// it by the settings view's one rule (`settings-view`: *Settings Persist by
/// One Rule*): only when the edit is committed — Enter, or focus leaving the
/// field — and never on a keystroke. A draft the setting does not accept is
/// refused on the field without being written; Escape abandons the edit.
///
/// A numeric field is a text input with a numeric keyboard rather than
/// `type="number"`, which reports an empty value for a partial entry — hiding
/// what was typed from the message that says what is wrong with it — and lets
/// a scroll wheel change the value without the user noticing.
export function CommittedField<T>({
    id,
    stored,
    format,
    parse,
    equals,
    onCommit,
    placeholder,
    numeric,
    disabled,
    ariaLabel,
}: CommittedFieldProps<T>) {
    const [draft, setDraft] = useState(() => format(stored))
    const [error, setError] = useState<string | null>(null)
    const inputRef = useRef<HTMLInputElement>(null)

    // Escape resets the draft and blurs — but blur dispatches synchronously,
    // before the reset has flushed, so the blur handler's closure would still
    // see the abandoned draft and commit it. This tells it to stand down for
    // that one blur (the pattern `WorkspaceRow`'s rename field established).
    const abandoningRef = useRef(false)

    // Take up a stored value that changed elsewhere — this field's own write,
    // or another window's — unless the user is mid-edit here.
    useEffect(() => {
        if (document.activeElement !== inputRef.current) setDraft(format(stored))
        // `format` is a pure presentation of `stored`.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [stored])

    const commit = async () => {
        const decision = commitDecision(draft, stored, parse, equals)
        if (decision.kind === "unchanged") {
            setError(null)
            setDraft(format(stored))
            return
        }
        if (decision.kind === "invalid") {
            setError(decision.message)
            return
        }
        setError(null)
        try {
            await onCommit(decision.value)
        } catch (err) {
            setDraft(format(stored))
            setError(`Couldn't save this — ${prettifyError(err)}`)
        }
    }

    return (
        <div className="settings-field-control">
            <input
                ref={inputRef}
                id={id}
                className="settings-text-input"
                value={draft}
                placeholder={placeholder}
                inputMode={numeric ? "numeric" : undefined}
                autoComplete="off"
                spellCheck={false}
                disabled={disabled}
                aria-label={ariaLabel}
                aria-invalid={error ? true : undefined}
                onChange={(e) => setDraft(e.target.value)}
                onBlur={() => {
                    if (abandoningRef.current) {
                        abandoningRef.current = false
                        return
                    }
                    void commit()
                }}
                onKeyDown={(e) => {
                    if (e.key === "Enter") {
                        e.preventDefault()
                        e.currentTarget.blur()
                    } else if (e.key === "Escape") {
                        // This field consumes Escape — keep it from reaching
                        // the app-level fallback that closes Settings.
                        e.stopPropagation()
                        abandoningRef.current = true
                        setDraft(format(stored))
                        setError(null)
                        e.currentTarget.blur()
                    }
                }}
            />
            {error && (
                <p className="settings-error settings-row-error" role="alert">
                    {error}
                </p>
            )}
        </div>
    )
}
