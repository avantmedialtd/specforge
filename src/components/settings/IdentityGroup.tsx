import { useEffect, useState } from "react"
import { getIdentity, setDisplayName, setIdentityAliases } from "../../api"
import { parseText } from "../../settingsFields"
import type { Author, IdentityInfo } from "../../types"
import { prettifyError } from "../errors"
import { CommittedField } from "./CommittedField"
import { SettingsRow } from "./SettingsRow"

/// Normalised attribution key for an author — email first, else name, lowered.
/// Mirrors `normalized_key` in `crates/openspec-core/src/identity.rs`.
function authorKey(a: Author): string {
    return (a.email ?? a.name ?? "").trim().toLowerCase()
}

function authorLabel(a: Author): string {
    if (a.name && a.email) return `${a.name} <${a.email}>`
    return a.name ?? a.email ?? "Unknown"
}

/// Build an `Author` from raw form fields, or `null` when both are blank
/// (mirrors the core "no usable identity" rule). Trims; omits empty components.
function makeAuthor(name: string, email: string): Author | null {
    const n = name.trim()
    const e = email.trim()
    if (!n && !e) return null
    return { ...(n ? { name: n } : {}), ...(e ? { email: e } : {}) }
}

/// A tiny name+email form that yields an `Author` on submit — the free-form add
/// used for your own identities.
function AddIdentityForm({ onAdd, label }: { onAdd: (a: Author) => void; label: string }) {
    const [name, setName] = useState("")
    const [email, setEmail] = useState("")
    const submit = () => {
        const a = makeAuthor(name, email)
        if (!a) return
        onAdd(a)
        setName("")
        setEmail("")
    }
    return (
        <div className="identity-add-form">
            <input
                className="settings-text-input"
                value={name}
                placeholder="Name"
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") submit()
                }}
                aria-label="Identity name"
            />
            <input
                className="settings-text-input"
                value={email}
                placeholder="email@example.com"
                onChange={(e) => setEmail(e.target.value)}
                onKeyDown={(e) => {
                    if (e.key === "Enter") submit()
                }}
                aria-label="Identity email"
            />
            <button className="btn-secondary" onClick={submit} disabled={!name.trim() && !email.trim()}>
                {label}
            </button>
        </div>
    )
}

/// Settings → Identity: who SpecForge attributes accomplishments to. The
/// canonical developer ("you") and the git identities that fold onto them —
/// the only attribution the app stores. Every other author is presented by
/// their raw git identity, with no naming or merging affordance
/// (`developer-identity`: *Named People Roster* was removed).
export function IdentityGroup() {
    const [info, setInfo] = useState<IdentityInfo | null>(null)
    // Why the last change to the identity list did not take.
    const [listError, setListError] = useState<string | null>(null)

    const reload = async () => {
        const next = await getIdentity().catch(() => null)
        if (next) setInfo(next)
    }
    useEffect(() => {
        void reload()
    }, [])

    if (!info) return <p className="settings-empty">Loading…</p>

    const aliases = info.config.aliases
    const aliasKeys = new Set(aliases.map(authorKey))
    const suggestions = info.candidates.filter((c) => !aliasKeys.has(authorKey(c)))

    const writeAliases = async (next: Author[], failure: string) => {
        setListError(null)
        try {
            await setIdentityAliases(next)
        } catch (err) {
            setListError(`${failure} — ${prettifyError(err)}`)
        }
        await reload()
    }
    const addAlias = (a: Author) => writeAliases([...aliases, a], "Couldn't add this identity")
    const removeAlias = (key: string) =>
        writeAliases(
            aliases.filter((a) => authorKey(a) !== key),
            "Couldn't remove this identity",
        )

    return (
        <>
            <p className="settings-help">
                Who you are, resolved from your <code>git</code> identity. Accomplishments across
                every OpenSpec workspace are attributed to these identities — fold in any extra
                emails or name variants you commit under so they all count as you.
            </p>

            <SettingsRow
                layout="stacked"
                title="Display name"
                controlId="settings-display-name"
                control={
                    <CommittedField
                        id="settings-display-name"
                        stored={info.config.displayName ?? null}
                        format={(name) => name ?? ""}
                        parse={parseText}
                        placeholder={info.config.aliases[0]?.name ?? "You"}
                        onCommit={async (name) => {
                            await setDisplayName(name)
                            await reload()
                        }}
                    />
                }
            />

            <div className="identity-group">
                <span className="settings-field-label">Your identities</span>
                {aliases.length === 0 ? (
                    <p className="settings-empty">None yet — add one below or from the detected list.</p>
                ) : (
                    <ul className="identity-list">
                        {aliases.map((a, i) => (
                            <li key={authorKey(a) || i} className="identity-row">
                                <span className="identity-label">
                                    {authorLabel(a)}
                                    {i === 0 && <span className="chip identity-primary">primary</span>}
                                </span>
                                <button
                                    className="btn-remove"
                                    onClick={() => void removeAlias(authorKey(a))}
                                    disabled={aliases.length === 1}
                                    title={
                                        aliases.length === 1
                                            ? "Keep at least one identity"
                                            : "Remove this identity"
                                    }
                                >
                                    Remove
                                </button>
                            </li>
                        ))}
                    </ul>
                )}
                <AddIdentityForm onAdd={(a) => void addAlias(a)} label="+ Add identity" />
                {listError && (
                    <p className="settings-error" role="alert">
                        {listError}
                    </p>
                )}
            </div>

            {suggestions.length > 0 && (
                <div className="identity-group">
                    <span className="settings-field-label">Detected git identities</span>
                    <ul className="identity-list">
                        {suggestions.map((a, i) => (
                            <li key={authorKey(a) || i} className="identity-row">
                                <span className="identity-label">{authorLabel(a)}</span>
                                <button className="btn-secondary" onClick={() => void addAlias(a)}>
                                    + This is me
                                </button>
                            </li>
                        ))}
                    </ul>
                </div>
            )}
        </>
    )
}
