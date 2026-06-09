import { useEffect, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import {
    getGamificationEnabled,
    getIdentity,
    getLaunchOnLogin,
    getNotificationsEnabled,
    getTreatmentLocker,
    getWslPollIntervalSecs,
    observedAuthors,
    registerWorkspace,
    setDisplayName,
    setEquippedTreatment,
    setGamificationEnabled,
    setIdentityAliases,
    setLaunchOnLogin,
    setNotificationsEnabled,
    setPeople,
    setWorkspacePresentation,
    setWslPollIntervalSecs,
    unregisterWorkspace,
} from "../api"
import { Close } from "./icons"
import {
    PALETTE_COLORS,
    type Author,
    type IdentityInfo,
    type PaletteColor,
    type Person,
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
    // `null` = not applicable (non-Windows) → the WSL section stays hidden.
    const [wslPollSecs, setWslPollSecs] = useState<number | null>(null)
    const [addError, setAddError] = useState<string | null>(null)
    const [busy, setBusy] = useState(false)

    useEffect(() => {
        let cancelled = false
        Promise.all([
            getLaunchOnLogin().catch(() => false),
            getNotificationsEnabled().catch(() => true),
            getGamificationEnabled().catch(() => false),
            getWslPollIntervalSecs().catch(() => null),
        ]).then(([launch, notif, game, wslPoll]) => {
            if (cancelled) return
            setLaunch(launch)
            setNotifs(notif)
            setGamification(game)
            setWslPollSecs(wslPoll)
        })
        return () => {
            cancelled = true
        }
    }, [])

    const handleWslPollChange = async (secs: number) => {
        if (!Number.isFinite(secs) || secs < 1) return
        const previous = wslPollSecs
        setWslPollSecs(secs)
        try {
            await setWslPollIntervalSecs(secs)
        } catch (err) {
            setWslPollSecs(previous)
            console.warn("failed to update WSL poll interval", err)
        }
    }

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

            {wslPollSecs != null && (
                <section className="settings-section">
                    <h2>WSL workspaces</h2>
                    <p className="settings-help">
                        Workspaces stored in the WSL filesystem (
                        <code>\\wsl.localhost\…</code>) are watched by polling,
                        because Windows receives no change events across the
                        share. How often to re-scan, in seconds.
                    </p>
                    <label className="settings-toggle-row">
                        <input
                            type="number"
                            min={1}
                            step={1}
                            value={wslPollSecs}
                            onChange={(e) =>
                                handleWslPollChange(parseInt(e.target.value, 10))
                            }
                        />
                        <span>Poll interval (seconds)</span>
                    </label>
                </section>
            )}
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

/// Build an `Author` from raw form fields, or `null` when both are blank
/// (mirrors the core "no usable identity" rule). Trims; omits empty components.
function makeAuthor(name: string, email: string): Author | null {
    const n = name.trim()
    const e = email.trim()
    if (!n && !e) return null
    return { ...(n ? { name: n } : {}), ...(e ? { email: e } : {}) }
}

/// Client mirror of `assign_identity`: a new roster with `author`'s key removed
/// from every other person, then appended to `target` (deduped). Keeps the
/// stored roster single-assigned; the core still enforces you-precedence.
function assignIdentity(people: Person[], target: number, author: Author): Person[] {
    const key = authorKey(author)
    if (!key) return people
    return people.map((p, i) => {
        const without = p.identities.filter((a) => authorKey(a) !== key)
        return i === target ? { ...p, identities: [...without, author] } : { ...p, identities: without }
    })
}

function personLabel(p: Person, i: number): string {
    return (
        p.displayName?.trim() ||
        (p.identities[0] ? authorLabel(p.identities[0]) : `Person ${i + 1}`)
    )
}

/// A tiny name+email form that yields an `Author` on submit — the free-form add
/// used both for your own identities and for a roster person's.
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
            <button
                className="btn-secondary"
                onClick={submit}
                disabled={!name.trim() && !email.trim()}
            >
                {label}
            </button>
        </div>
    )
}

/// One roster person: an editable name, their folded identities, and a free-form
/// add. Pure presentation — all mutations bubble up to the section's savers.
function PersonCard({
    person,
    index,
    onRename,
    onRemove,
    onRemoveIdentity,
    onAddIdentity,
}: {
    person: Person
    index: number
    onRename: (name: string) => void
    onRemove: () => void
    onRemoveIdentity: (key: string) => void
    onAddIdentity: (a: Author) => void
}) {
    const [name, setName] = useState(person.displayName ?? "")
    useEffect(() => {
        setName(person.displayName ?? "")
    }, [person.displayName])
    return (
        <div className="identity-group person-card">
            <div className="person-head">
                <input
                    className="settings-text-input"
                    value={name}
                    placeholder={personLabel(person, index)}
                    onChange={(e) => setName(e.target.value)}
                    onBlur={() => onRename(name)}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") (e.target as HTMLInputElement).blur()
                    }}
                    aria-label="Person display name"
                />
                <button className="btn-remove" onClick={onRemove} title="Remove this person">
                    Remove
                </button>
            </div>
            {person.identities.length === 0 ? (
                <p className="settings-empty">No identities yet.</p>
            ) : (
                <ul className="identity-list">
                    {person.identities.map((a, j) => (
                        <li key={authorKey(a) || j} className="identity-row">
                            <span className="identity-label">{authorLabel(a)}</span>
                            <button
                                className="btn-remove"
                                onClick={() => onRemoveIdentity(authorKey(a))}
                            >
                                Remove
                            </button>
                        </li>
                    ))}
                </ul>
            )}
            <AddIdentityForm onAdd={onAddIdentity} label="+ Add identity" />
        </div>
    )
}

/// Settings → Identity + People: who SpecForge attributes accomplishments to.
/// The Identity section is the canonical developer ("you"); the People section
/// names and merges other contributors on the leaderboard.
function IdentitySection() {
    const [info, setInfo] = useState<IdentityInfo | null>(null)
    const [draftName, setDraftName] = useState("")
    const [observed, setObserved] = useState<Author[]>([])

    const reload = async () => {
        const next = await getIdentity().catch(() => null)
        if (next) {
            setInfo(next)
            setDraftName(next.config.displayName ?? "")
        }
    }
    useEffect(() => {
        void reload()
        void observedAuthors()
            .then(setObserved)
            .catch(() => setObserved([]))
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

    const people = info.people
    const assignedKeys = new Set(people.flatMap((p) => p.identities.map(authorKey)))
    const unassignedObserved = observed.filter((a) => {
        const k = authorKey(a)
        return k.length > 0 && !assignedKeys.has(k)
    })

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

    const savePeople = async (next: Person[]) => {
        await setPeople(next).catch((e) => console.warn("failed to save roster", e))
        await reload()
    }
    const addPerson = () => void savePeople([...people, { displayName: null, identities: [] }])
    const removePerson = (i: number) => void savePeople(people.filter((_, idx) => idx !== i))
    const renamePerson = (i: number, name: string) =>
        void savePeople(
            people.map((p, idx) => (idx === i ? { ...p, displayName: name.trim() || null } : p)),
        )
    const addIdentityToPerson = (i: number, a: Author) =>
        void savePeople(assignIdentity(people, i, a))
    const removeIdentityFromPerson = (i: number, key: string) =>
        void savePeople(
            people.map((p, idx) =>
                idx === i
                    ? { ...p, identities: p.identities.filter((x) => authorKey(x) !== key) }
                    : p,
            ),
        )
    const createPersonWith = (a: Author) =>
        void savePeople(
            assignIdentity(
                [...people, { displayName: a.name ?? null, identities: [] }],
                people.length,
                a,
            ),
        )

    return (
        <>
            <section className="settings-section">
                <h2>Identity</h2>
                <p className="settings-help">
                    Who you are, resolved from your <code>git</code> identity.
                    Accomplishments across every OpenSpec workspace are attributed
                    to these identities — fold in any extra emails or name variants
                    you commit under so they all count as you.
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
                            None yet — add one below or from the detected list.
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
                    <AddIdentityForm onAdd={(a) => void addAlias(a)} label="+ Add identity" />
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

            <section className="settings-section">
                <h2>People</h2>
                <p className="settings-help">
                    Name and merge other contributors on the leaderboard. Fold a
                    teammate's several git identities into one person and their
                    ships, tasks, and commits are summed under the name you choose.
                    This only changes how the leaderboard reads — it never affects
                    your own season standing.
                </p>

                {people.length === 0 ? (
                    <p className="settings-empty">
                        No people yet. Add one, or assign an observed author below.
                    </p>
                ) : (
                    people.map((p, i) => (
                        <PersonCard
                            key={i}
                            person={p}
                            index={i}
                            onRename={(name) => renamePerson(i, name)}
                            onRemove={() => removePerson(i)}
                            onRemoveIdentity={(key) => removeIdentityFromPerson(i, key)}
                            onAddIdentity={(a) => addIdentityToPerson(i, a)}
                        />
                    ))
                )}

                <button className="btn-secondary" onClick={addPerson}>
                    + Add person
                </button>

                {unassignedObserved.length > 0 && (
                    <div className="identity-group">
                        <span className="settings-field-label">Observed authors</span>
                        <ul className="identity-list">
                            {unassignedObserved.map((a) => (
                                <li key={authorKey(a)} className="identity-row">
                                    <span className="identity-label">{authorLabel(a)}</span>
                                    {people.length > 0 ? (
                                        <select
                                            className="settings-text-input"
                                            value=""
                                            onChange={(e) => {
                                                const idx = Number(e.target.value)
                                                if (!Number.isNaN(idx)) addIdentityToPerson(idx, a)
                                            }}
                                            aria-label="Assign author to a person"
                                        >
                                            <option value="" disabled>
                                                Assign to…
                                            </option>
                                            {people.map((p, i) => (
                                                <option key={i} value={i}>
                                                    {personLabel(p, i)}
                                                </option>
                                            ))}
                                        </select>
                                    ) : (
                                        <button
                                            className="btn-secondary"
                                            onClick={() => createPersonWith(a)}
                                        >
                                            + New person
                                        </button>
                                    )}
                                </li>
                            ))}
                        </ul>
                    </div>
                )}
            </section>
        </>
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
