import { useEffect, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import {
    getGamificationEnabled,
    getIdentity,
    getLaunchOnLogin,
    getNotificationsEnabled,
    getTreatmentLocker,
    registerWorkspace,
    setDisplayName,
    setEquippedTreatment,
    setGamificationEnabled,
    setIdentityAliases,
    setLaunchOnLogin,
    setNotificationsEnabled,
    setWorkspacePresentation,
    unregisterWorkspace,
} from "../api"
import { Close } from "./icons"
import {
    PALETTE_COLORS,
    type Author,
    type IdentityInfo,
    type PaletteColor,
    type RegisteredWorkspace,
    type TreatmentDescriptor,
    type TreatmentLocker,
} from "../types"

interface SettingsViewProps {
    workspaces: RegisteredWorkspace[]
    onWorkspacesChanged: () => Promise<void>
    onClose: () => void
}

export function SettingsView({
    workspaces,
    onWorkspacesChanged,
    onClose,
}: SettingsViewProps) {
    const [launchAtLogin, setLaunch] = useState<boolean | null>(null)
    const [notifications, setNotifs] = useState<boolean | null>(null)
    const [gamification, setGamification] = useState<boolean | null>(null)
    const [addError, setAddError] = useState<string | null>(null)
    const [busy, setBusy] = useState(false)

    useEffect(() => {
        let cancelled = false
        Promise.all([
            getLaunchOnLogin().catch(() => false),
            getNotificationsEnabled().catch(() => true),
            getGamificationEnabled().catch(() => false),
        ]).then(([launch, notif, game]) => {
            if (cancelled) return
            setLaunch(launch)
            setNotifs(notif)
            setGamification(game)
        })
        return () => {
            cancelled = true
        }
    }, [])

    const handleGamificationToggle = async () => {
        if (gamification == null) return
        const next = !gamification
        setGamification(next)
        try {
            await setGamificationEnabled(next)
        } catch (err) {
            setGamification(!next)
            console.warn("failed to update gamification-enabled", err)
        }
    }

    const handleAdd = async () => {
        setAddError(null)
        setBusy(true)
        try {
            const selected = await open({
                multiple: false,
                directory: true,
                title: "Choose an OpenSpec workspace folder",
            })
            if (typeof selected === "string") {
                try {
                    await registerWorkspace(selected)
                    await onWorkspacesChanged()
                } catch (err) {
                    setAddError(prettifyError(err))
                }
            }
        } catch (err) {
            setAddError(prettifyError(err))
        } finally {
            setBusy(false)
        }
    }

    const handleRemove = async (uri: string) => {
        try {
            await unregisterWorkspace(uri)
            await onWorkspacesChanged()
        } catch (err) {
            console.warn("failed to unregister", uri, err)
        }
    }

    const handleLaunchToggle = async () => {
        if (launchAtLogin == null) return
        const next = !launchAtLogin
        setLaunch(next)
        try {
            await setLaunchOnLogin(next)
        } catch (err) {
            setLaunch(!next)
            console.warn("failed to update launch-at-login", err)
        }
    }

    const handleNotifToggle = async () => {
        if (notifications == null) return
        const next = !notifications
        setNotifs(next)
        try {
            await setNotificationsEnabled(next)
        } catch (err) {
            setNotifs(!next)
            console.warn("failed to update notifications-enabled", err)
        }
    }

    return (
        <div className="settings-view">
            <header className="settings-header">
                <h1>Settings</h1>
                <button
                    className="settings-close"
                    onClick={onClose}
                    aria-label="Close settings"
                    title="Close settings"
                >
                    <Close width={14} height={14} />
                </button>
            </header>

            <section className="settings-section">
                <h2>Workspaces</h2>
                <p className="settings-help">
                    Folders containing an <code>openspec/</code> directory. Add
                    each workspace whose specs you want to monitor.
                </p>

                {workspaces.length === 0 ? (
                    <p className="settings-empty">No workspaces registered yet.</p>
                ) : (
                    <ul className="workspaces-list">
                        {workspaces.map((ws) => (
                            <WorkspaceRow
                                key={ws.uri}
                                ws={ws}
                                onRemove={() => handleRemove(ws.uri)}
                            />
                        ))}
                    </ul>
                )}
                <button
                    className="btn-primary"
                    onClick={handleAdd}
                    disabled={busy}
                >
                    {busy ? "Adding…" : "+ Add workspace"}
                </button>
                {addError && <p className="settings-error">{addError}</p>}
            </section>

            <section className="settings-section">
                <h2>Gamification</h2>
                <p className="settings-help">
                    Turn the Dashboard's progress game on — seasons and the
                    battle pass, your streak, the contribution heatmap, the
                    leaderboard, and badge finishes. Off by default; the
                    Dashboard shows just its analytics until you enable it.
                </p>
                <label className="settings-toggle-row">
                    <input
                        type="checkbox"
                        checked={gamification ?? false}
                        disabled={gamification == null}
                        onChange={handleGamificationToggle}
                    />
                    <span>Show the gamified progress layer</span>
                </label>
            </section>

            <IdentitySection />

            {gamification && <BadgeFinishesSection />}

            <section className="settings-section">
                <h2>Notifications</h2>
                <label className="settings-toggle-row">
                    <input
                        type="checkbox"
                        checked={notifications ?? false}
                        disabled={notifications == null}
                        onChange={handleNotifToggle}
                    />
                    <span>Show notifications for new and archived changes</span>
                </label>
            </section>

            <section className="settings-section">
                <h2>Startup</h2>
                <label className="settings-toggle-row">
                    <input
                        type="checkbox"
                        checked={launchAtLogin ?? false}
                        disabled={launchAtLogin == null}
                        onChange={handleLaunchToggle}
                    />
                    <span>Launch at login</span>
                </label>
            </section>
        </div>
    )
}

/// Normalised attribution key for an author — email first, else name, lowered.
/// Mirrors `normalized_key` in `crates/openspec-core/src/identity.rs`.
function authorKey(a: Author): string {
    return (a.email ?? a.name ?? "").trim().toLowerCase()
}

function authorLabel(a: Author): string {
    if (a.name && a.email) return `${a.name} <${a.email}>`
    return a.name ?? a.email ?? "Unknown"
}

/// Settings → Identity: who SpecForge attributes your accomplishments to.
/// Shows the canonical display name, your current identities, and the git
/// identities detected across registered workspaces (offered to fold in as you).
function IdentitySection() {
    const [info, setInfo] = useState<IdentityInfo | null>(null)
    const [draftName, setDraftName] = useState("")

    const reload = async () => {
        const next = await getIdentity().catch(() => null)
        if (next) {
            setInfo(next)
            setDraftName(next.config.displayName ?? "")
        }
    }
    useEffect(() => {
        void reload()
    }, [])

    if (!info) {
        return (
            <section className="settings-section">
                <h2>Identity</h2>
                <p className="settings-empty">Loading…</p>
            </section>
        )
    }

    const aliases = info.config.aliases
    const aliasKeys = new Set(aliases.map(authorKey))
    const suggestions = info.candidates.filter((c) => !aliasKeys.has(authorKey(c)))

    const commitName = async () => {
        const next = draftName.trim()
        if ((info.config.displayName ?? "") === next) return
        await setDisplayName(next.length ? next : null).catch((e) =>
            console.warn("failed to set display name", e),
        )
        await reload()
    }
    const addAlias = async (a: Author) => {
        await setIdentityAliases([...aliases, a]).catch((e) =>
            console.warn("failed to add alias", e),
        )
        await reload()
    }
    const removeAlias = async (key: string) => {
        await setIdentityAliases(aliases.filter((a) => authorKey(a) !== key)).catch((e) =>
            console.warn("failed to remove alias", e),
        )
        await reload()
    }

    return (
        <section className="settings-section">
            <h2>Identity</h2>
            <p className="settings-help">
                Who you are, resolved from your <code>git</code> identity.
                Accomplishments across every OpenSpec workspace are attributed to
                these identities — fold in any extra emails or name variants you
                commit under so they all count as you.
            </p>

            <label className="settings-field">
                <span className="settings-field-label">Display name</span>
                <input
                    className="settings-text-input"
                    value={draftName}
                    placeholder={info.config.aliases[0]?.name ?? "You"}
                    onChange={(e) => setDraftName(e.target.value)}
                    onBlur={() => void commitName()}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") (e.target as HTMLInputElement).blur()
                    }}
                    aria-label="Canonical display name"
                />
            </label>

            <div className="identity-group">
                <span className="settings-field-label">Your identities</span>
                {aliases.length === 0 ? (
                    <p className="settings-empty">
                        None yet — add one from the detected list below.
                    </p>
                ) : (
                    <ul className="identity-list">
                        {aliases.map((a, i) => (
                            <li key={authorKey(a) || i} className="identity-row">
                                <span className="identity-label">
                                    {authorLabel(a)}
                                    {i === 0 && (
                                        <span className="chip identity-primary">primary</span>
                                    )}
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
            </div>

            {suggestions.length > 0 && (
                <div className="identity-group">
                    <span className="settings-field-label">Detected git identities</span>
                    <ul className="identity-list">
                        {suggestions.map((a, i) => (
                            <li key={authorKey(a) || i} className="identity-row">
                                <span className="identity-label">{authorLabel(a)}</span>
                                <button
                                    className="btn-secondary"
                                    onClick={() => void addAlias(a)}
                                >
                                    + This is me
                                </button>
                            </li>
                        ))}
                    </ul>
                </div>
            )}
        </section>
    )
}

/// A treatment finish swatch — same global `.treatment` styling the Dashboard
/// uses, so the locker reads identically in both places.
function FinishSwatch({ t, size = 30 }: { t: TreatmentDescriptor; size?: number }) {
    const hue = (t.palette[0] ?? 0) * 30
    const hue2 = (t.palette[1] ?? 6) * 30
    return (
        <span
            className={`treatment treatment--${t.effect} treatment--${t.rarity}`}
            aria-hidden
            style={
                {
                    width: size,
                    height: size,
                    "--treat-hue": `${hue}`,
                    "--treat-hue2": `${hue2}`,
                } as React.CSSProperties
            }
        />
    )
}

/// Settings → Badge finishes: the treatment wardrobe. Every finish unlocked by
/// climbing seasons' battle passes; click one to wear it on your profile
/// avatar, or click the equipped one to take it off. Cosmetic only.
function BadgeFinishesSection() {
    const [locker, setLocker] = useState<TreatmentLocker | null>(null)
    // In-session equipped override for instant feedback before the reload lands.
    const [equippedId, setEquippedId] = useState<string | null | undefined>(undefined)

    const reload = async () => {
        const next = await getTreatmentLocker().catch(() => null)
        if (next) setLocker(next)
    }
    useEffect(() => {
        void reload()
    }, [])

    const currentEquipped =
        equippedId === undefined ? (locker?.equipped?.id ?? null) : equippedId

    const equip = async (id: string | null) => {
        setEquippedId(id)
        await setEquippedTreatment(id).catch((e) =>
            console.warn("failed to equip treatment", e),
        )
        await reload()
    }

    return (
        <section className="settings-section">
            <h2>Badge finishes</h2>
            <p className="settings-help">
                Cosmetic finishes you unlock by climbing each season's battle
                pass. Equip one to style your profile avatar on the Dashboard —
                click the worn finish again to remove it.
            </p>

            {!locker || locker.unlocked.length === 0 ? (
                <p className="settings-empty">
                    No finishes yet — they unlock as your season score climbs the
                    battle-pass tiers.
                </p>
            ) : (
                <div className="finishes-grid">
                    {locker.unlocked.map((t) => (
                        <button
                            key={t.id}
                            type="button"
                            className={`finishes-item${
                                currentEquipped === t.id ? " equipped" : ""
                            }`}
                            onClick={() =>
                                void equip(currentEquipped === t.id ? null : t.id)
                            }
                            title={`${t.rarity} · ${t.effect}`}
                        >
                            <FinishSwatch t={t} />
                            <span className="finishes-meta">
                                <span className="finishes-effect">{t.effect}</span>
                                <span className={`finishes-rarity finishes-rarity--${t.rarity}`}>
                                    {t.rarity}
                                </span>
                            </span>
                            {currentEquipped === t.id && (
                                <span className="finishes-equipped-tag">equipped</span>
                            )}
                        </button>
                    ))}
                </div>
            )}
        </section>
    )
}

interface WorkspaceRowProps {
    ws: RegisteredWorkspace
    onRemove: () => void
}

function WorkspaceRow({ ws, onRemove }: WorkspaceRowProps) {
    // Local copy of the rename input so editing is responsive; commit on
    // blur and Enter. Cleared input becomes `null` server-side so the row
    // reverts to its basename-derived default.
    const [draftName, setDraftName] = useState<string>(ws.displayName ?? "")

    // If the underlying workspace's persisted name changes (e.g. another
    // refresh path updated it), pull the new value in unless the user is
    // mid-edit. We approximate "mid-edit" by checking focus before applying.
    useEffect(() => {
        setDraftName(ws.displayName ?? "")
    }, [ws.displayName, ws.uri])

    const commitName = async () => {
        const next = draftName.trim()
        const persisted = ws.displayName ?? ""
        if (next === persisted) return
        try {
            await setWorkspacePresentation(
                ws.uri,
                ws.repoId,
                next.length === 0 ? null : next,
                ws.color,
            )
        } catch (err) {
            console.warn("failed to set display name", err)
            // Snap back to the persisted value on failure.
            setDraftName(ws.displayName ?? "")
        }
    }

    const setColor = async (color: PaletteColor | null) => {
        if (color === ws.color) return
        try {
            await setWorkspacePresentation(
                ws.uri,
                ws.repoId,
                ws.displayName,
                color,
            )
        } catch (err) {
            console.warn("failed to set workspace colour", err)
        }
    }

    return (
        <li
            className={`workspace-row${ws.isMissing ? " missing" : ""}`}
        >
            <div className="workspace-info">
                <div className="workspace-name">
                    <input
                        className="workspace-name-input"
                        value={draftName}
                        placeholder={ws.name}
                        onChange={(e) => setDraftName(e.target.value)}
                        onBlur={() => {
                            void commitName()
                        }}
                        onKeyDown={(e) => {
                            if (e.key === "Enter") {
                                e.preventDefault()
                                ;(e.target as HTMLInputElement).blur()
                            } else if (e.key === "Escape") {
                                setDraftName(ws.displayName ?? "")
                                ;(e.target as HTMLInputElement).blur()
                            }
                        }}
                        aria-label={`Display name for ${ws.name}`}
                    />
                    {ws.isMissing && (
                        <span className="chip chip--warn">missing</span>
                    )}
                </div>
                <div className="workspace-path" title={ws.uri}>
                    {ws.uri}
                </div>
                <div
                    className="workspace-palette"
                    role="radiogroup"
                    aria-label="Workspace tint colour"
                >
                    <button
                        type="button"
                        className={`palette-swatch palette-swatch--none${
                            ws.color === null ? " selected" : ""
                        }`}
                        onClick={() => void setColor(null)}
                        aria-label="No tint"
                        aria-pressed={ws.color === null}
                        title="No tint"
                    />
                    {PALETTE_COLORS.map((token) => (
                        <button
                            key={token}
                            type="button"
                            className={`palette-swatch palette-swatch--${token}${
                                ws.color === token ? " selected" : ""
                            }`}
                            onClick={() => void setColor(token)}
                            aria-label={`Tint colour ${token}`}
                            aria-pressed={ws.color === token}
                            title={token}
                        />
                    ))}
                </div>
            </div>
            <button
                className="btn-remove"
                onClick={onRemove}
                aria-label={`Remove ${ws.name}`}
            >
                Remove
            </button>
        </li>
    )
}

function prettifyError(err: unknown): string {
    const s = String(err)
    // Tauri wraps command errors in `IpcMessage:Error("...")` shells —
    // unwrap to the readable inner message where possible.
    const inner = s.match(/Error\("(.+)"\)/)?.[1] ?? s
    return inner.replace(/^[A-Za-z]+::/, "")
}
