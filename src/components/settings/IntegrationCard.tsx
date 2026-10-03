import type { ReactNode } from "react"
import { SettingsRow } from "./SettingsRow"
import { Switch } from "./Switch"

interface IntegrationCardProps {
    /// A stable id the card derives its control ids from.
    id: string
    name: string
    /// What turning it on reads, and where it sends requests.
    description: ReactNode
    /// `null` while the stored value loads.
    enabled: boolean | null
    onToggle: (next: boolean) => void
    /// Why the last switch write did not take.
    error?: string | null
    /// Set when the integration is off but a credential is still stored, so the
    /// card can say so and offer to remove it without turning the integration
    /// on — which would start polling with that very credential.
    storedCredential?: { label: string; onRemove: () => void; busy: boolean; error: string | null } | null
    /// What the card adds while the integration is on — its credential form,
    /// setup instructions and panel slot. Never rendered while it is off.
    children?: ReactNode
}

/// One opt-in integration (`settings-view`: *Opt-In Integrations Collapse
/// While Off*). Off, it is a single row — name, description, switch — plus a
/// line about a stored credential if one is left behind. On, it opens to show
/// what it needs.
export function IntegrationCard({
    id,
    name,
    description,
    enabled,
    onToggle,
    error,
    storedCredential,
    children,
}: IntegrationCardProps) {
    const on = enabled === true
    return (
        <section className={`integration-card${on ? " integration-card--on" : ""}`} aria-label={name}>
            <SettingsRow
                title={name}
                controlId={`${id}-switch`}
                description={description}
                error={error}
                control={
                    <Switch
                        id={`${id}-switch`}
                        checked={on}
                        disabled={enabled === null}
                        onChange={onToggle}
                    />
                }
            />
            {!on && storedCredential && (
                <div className="integration-card-stored">
                    <span>{storedCredential.label}</span>
                    <button
                        type="button"
                        className="btn-remove"
                        onClick={storedCredential.onRemove}
                        disabled={storedCredential.busy}
                    >
                        {storedCredential.busy ? "Removing…" : "Remove"}
                    </button>
                    {storedCredential.error && (
                        <p className="settings-error settings-row-error" role="alert">
                            {storedCredential.error}
                        </p>
                    )}
                </div>
            )}
            {on && children && <div className="integration-card-body">{children}</div>}
        </section>
    )
}
