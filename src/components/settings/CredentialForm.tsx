import type { ReactNode } from "react"
import type { CredentialSource, StoredCredentialSource } from "../../types"

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

/// What one credential source is called, without ever naming a value. The
/// browser renders none of this (`settings-view`: *Provider Credential Editing
/// Is Desktop-Only*).
export function credentialSourceLabel(
    source: CredentialSource | StoredCredentialSource,
): string {
    switch (source) {
        case "environment":
            return "environment variables"
        case "secure":
            return "the operating-system credential store"
        case "legacy":
            return "legacy plaintext settings"
        case "none":
            return "none"
    }
}

interface CredentialSourcesProps {
    active: CredentialSource
    stored: StoredCredentialSource
}

/// The non-secret source state of an enabled native card. Active and stored
/// are separate lines because an environment override does not hide an
/// underlying secure or legacy entry, and the clear action beside a stored
/// entry removes only that entry (`settings-view`: *Opt-In Integrations
/// Collapse While Off*).
export function CredentialSources({ active, stored }: CredentialSourcesProps) {
    return (
        <dl className="credential-sources">
            <div className="credential-source">
                <dt>Active credential</dt>
                <dd>{credentialSourceLabel(active)}</dd>
            </div>
            <div className="credential-source">
                <dt>Stored credential</dt>
                <dd>{credentialSourceLabel(stored)}</dd>
            </div>
        </dl>
    )
}

interface StoredCredentialRowProps {
    /// What is stored and where, without its value.
    label: ReactNode
    onRemove: () => void
    busy: boolean
    error: string | null
}

/// The line a native card shows for a credential left behind, with the one
/// clear action. It is shown while the integration is off too, so removing the
/// credential never turns the integration on and starts polling with it.
export function StoredCredentialRow({
    label,
    onRemove,
    busy,
    error,
}: StoredCredentialRowProps) {
    return (
        <div className="integration-card-stored">
            <span>{label}</span>
            <button
                type="button"
                className="btn-remove"
                onClick={onRemove}
                disabled={busy}
            >
                {busy ? "Removing…" : "Remove"}
            </button>
            {error && (
                <p className="settings-error settings-row-error" role="alert">
                    {error}
                </p>
            )}
        </div>
    )
}

interface LegacyCredentialPromptProps {
    /// What the old source holds: "GitHub token" or "BitBucket credential
    /// pair".
    credential: string
}

/// A legacy plaintext credential still powers reads, but never a review write
/// (`pull-request-actions`: *Current State Authorises Every Draft and
/// Submission Boundary*). The prompt asks for an explicit secure re-save and
/// never fills a field with the old value.
export function LegacyCredentialPrompt({ credential }: LegacyCredentialPromptProps) {
    return (
        <p className="settings-help credential-legacy" role="status">
            This {credential} is stored as legacy plaintext in the settings file. Reads
            continue to use it, but review actions need it re-saved in the
            operating-system credential store before they become available. Enter it
            again above and save.
        </p>
    )
}
