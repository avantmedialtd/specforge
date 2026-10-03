import type { ReactNode } from "react"

interface CredentialInputProps {
    id: string
    label: string
    value: string
    onChange: (value: string) => void
    placeholder: string
    /// A secret is masked and never pre-filled — no command returns it.
    secret?: boolean
    /// Enter in this field saves, as it did before Settings was grouped.
    onEnter?: () => void
}

/// One field of a credential form: a label above its input.
export function CredentialInput({
    id,
    label,
    value,
    onChange,
    placeholder,
    secret,
    onEnter,
}: CredentialInputProps) {
    return (
        <div className="settings-field">
            <label className="settings-field-label" htmlFor={id}>
                {label}
            </label>
            <input
                id={id}
                className="settings-text-input"
                type={secret ? "password" : "text"}
                value={value}
                placeholder={placeholder}
                autoComplete="off"
                spellCheck={false}
                onChange={(e) => onChange(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter" && onEnter) onEnter()
                }}
            />
        </div>
    )
}

interface CredentialFormProps {
    children: ReactNode
    /// What saving does — which fields it replaces, and that an empty token
    /// removes the stored one.
    note: ReactNode
    saveLabel: string
    saving: boolean
    onSave: () => void
    /// The outcome of the last save. A failure is announced as an alert.
    message: { text: string; failed: boolean } | null
}

/// The credential fields of an integration that takes one, persisted only by
/// the explicit save action — never on blur, so a half-typed token is never
/// sent anywhere (`settings-view`: *Settings Persist by One Rule*).
export function CredentialForm({ children, note, saveLabel, saving, onSave, message }: CredentialFormProps) {
    return (
        <div className="credential-form">
            {children}
            <p className="settings-help">{note}</p>
            <div className="settings-choice-row">
                <button type="button" className="btn-secondary" onClick={onSave} disabled={saving}>
                    {saving ? "Saving…" : saveLabel}
                </button>
            </div>
            {message &&
                (message.failed ? (
                    <p className="settings-error settings-row-error" role="alert">
                        {message.text}
                    </p>
                ) : (
                    <p className="settings-help" role="status">
                        {message.text}
                    </p>
                ))}
        </div>
    )
}
