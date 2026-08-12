import { useEffect, useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import {
    getChatGptQuotaEnabled,
    getClaudeQuotaEnabled,
    getGamificationEnabled,
    getIdentity,
    getLaunchOnLogin,
    getNotificationsEnabled,
    getTreatmentLocker,
    getWebConfig,
    getWslPollIntervalSecs,
    isTauri,
    observedAuthors,
    registerWorkspace,
    setChatGptQuotaEnabled,
    setClaudeQuotaEnabled,
    setDisplayName,
    setEquippedTreatment,
    setGamificationEnabled,
    setIdentityAliases,
    setLaunchOnLogin,
    setNotificationsEnabled,
    setPeople,
    resolveTailscaleName,
    setWebEnabled,
    setWebPort,
    setWebTailscaleAllowedLogins,
    setWebTailscaleEnabled,
    setWebTailscaleName,
    setWorkspaceDisabled,
    setWorkspacePresentation,
    setWslPollIntervalSecs,
    unregisterWorkspace,
} from "../api"
import { prettifyError } from "./errors"
import { Close } from "./icons"
import { siblingsOf } from "../workspaceRows"
import {
    PALETTE_COLORS,
    type Author,
    type IdentityInfo,
    type PaletteColor,
    type Person,
    type RegisteredWorkspace,
    type TreatmentDescriptor,
    type TreatmentLocker,
    type WebServerConfig,
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
    const [quotaEnabled, setQuota] = useState<boolean | null>(null)
    const [chatGptQuotaEnabled, setChatGptQuota] = useState<boolean | null>(null)
    // `null` = not applicable (non-Windows) → the WSL section stays hidden.
    const [wslPollSecs, setWslPollSecs] = useState<number | null>(null)
    const [addError, setAddError] = useState<string | null>(null)
    const [busy, setBusy] = useState(false)
    // In the browser there is no native folder dialog, so the user types a path.
    const [pathInput, setPathInput] = useState("")

    useEffect(() => {
        let cancelled = false
        Promise.all([
            getLaunchOnLogin().catch(() => false),
            getNotificationsEnabled().catch(() => true),
            getGamificationEnabled().catch(() => false),
            getWslPollIntervalSecs().catch(() => null),
            getClaudeQuotaEnabled().catch(() => false),
            getChatGptQuotaEnabled().catch(() => false),
        ]).then(([launch, notif, game, wslPoll, quota, chatGptQuota]) => {
            if (cancelled) return
            setLaunch(launch)
            setNotifs(notif)
            setGamification(game)
            setWslPollSecs(wslPoll)
            setQuota(quota)
            setChatGptQuota(chatGptQuota)
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

    const handleQuotaToggle = async () => {
        if (quotaEnabled == null) return
        const next = !quotaEnabled
        setQuota(next)
        try {
            await setClaudeQuotaEnabled(next)
        } catch (err) {
            setQuota(!next)
            console.warn("failed to update claude-quota-enabled", err)
        }
    }

    const handleChatGptQuotaToggle = async () => {
        if (chatGptQuotaEnabled == null) return
        const next = !chatGptQuotaEnabled
        setChatGptQuota(next)
        try {
            await setChatGptQuotaEnabled(next)
        } catch (err) {
            setChatGptQuota(!next)
            console.warn("failed to update chatgpt-quota-enabled", err)
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

    // Web path-input variant of "add workspace": the backend `register_workspace`
    // takes a plain path string, so the browser just supplies it directly
    // (validated server-side: must exist and contain an `openspec/` directory).
    const handleAddPath = async () => {
        const path = pathInput.trim()
        if (!path) return
        setAddError(null)
        setBusy(true)
        try {
            await registerWorkspace(path)
            setPathInput("")
            await onWorkspacesChanged()
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
                                siblings={siblingsOf(ws, workspaces)}
                                onRemove={() => handleRemove(ws.uri)}
                            />
                        ))}
                    </ul>
                )}
                {isTauri() ? (
                    <button
                        className="btn-primary"
                        onClick={handleAdd}
                        disabled={busy}
                    >
                        {busy ? "Adding…" : "+ Add workspace"}
                    </button>
                ) : (
                    <div className="identity-add-form">
                        <input
                            className="settings-text-input"
                            value={pathInput}
                            placeholder="/path/to/your/openspec/workspace"
                            onChange={(e) => setPathInput(e.target.value)}
                            onKeyDown={(e) => {
                                if (e.key === "Enter") void handleAddPath()
                            }}
                            aria-label="Workspace folder path"
                        />
                        <button
                            className="btn-primary"
                            onClick={handleAddPath}
                            disabled={busy || !pathInput.trim()}
                        >
                            {busy ? "Adding…" : "+ Add"}
                        </button>
                    </div>
                )}
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

            {/* OS desktop notifications are a native-shell affordance — hidden
                in the browser, where the page can't raise system notifications. */}
            {isTauri() && (
                <section className="settings-section">
                    <h2>Notifications</h2>
                    <label className="settings-toggle-row">
                        <input
                            type="checkbox"
                            checked={notifications ?? false}
                            disabled={notifications == null}
                            onChange={handleNotifToggle}
                        />
                        <span>
                            Show notifications for new and archived changes
                        </span>
                    </label>
                </section>
            )}

            <section className="settings-section">
                <h2>Claude quota</h2>
                <p className="settings-help">
                    Show a small gauge of your Claude usage — the 5-hour and
                    weekly windows — in the sidebar footer. Reads your local
                    Claude Code login (read-only) to query Anthropic's usage
                    endpoint. Off by default; nothing is read or sent until you
                    enable it.
                </p>
                <label className="settings-toggle-row">
                    <input
                        type="checkbox"
                        checked={quotaEnabled ?? false}
                        disabled={quotaEnabled == null}
                        onChange={handleQuotaToggle}
                    />
                    <span>Show the Claude usage-quota gauge</span>
                </label>
            </section>

            <section className="settings-section">
                <h2>ChatGPT quota</h2>
                <p className="settings-help">
                    Show a small gauge of your ChatGPT usage — the 5-hour and
                    weekly windows — in the sidebar footer. Reads your local
                    Codex CLI login (read-only) to query ChatGPT's usage
                    endpoint. Off by default; nothing is read or sent until you
                    enable it.
                </p>
                <label className="settings-toggle-row">
                    <input
                        type="checkbox"
                        checked={chatGptQuotaEnabled ?? false}
                        disabled={chatGptQuotaEnabled == null}
                        onChange={handleChatGptQuotaToggle}
                    />
                    <span>Show the ChatGPT usage-quota gauge</span>
                </label>
            </section>

            {/* Launch-at-login lives in the OS (autostart) and the embedded
                web-server toggle configures a native process — both are
                desktop-only and absent from the browser skin. */}
            {isTauri() && (
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
            )}

            {isTauri() && <WebServerSection />}

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

/// Settings → Web UI (desktop only): the embedded local web-server toggle. When
/// enabled, the desktop app also serves the browser skin on a loopback port from
/// the same live state. Changes take effect at the next launch.
function WebServerSection() {
    const [config, setConfig] = useState<WebServerConfig | null>(null)
    // The tailnet name the server would currently trust (manual or discovered).
    const [resolvedName, setResolvedName] = useState<string | null>(null)
    // Local draft copies for the free-text overrides; committed on blur.
    const [nameDraft, setNameDraft] = useState("")
    const [loginsDraft, setLoginsDraft] = useState("")

    useEffect(() => {
        void getWebConfig()
            .then((c) => {
                setConfig(c)
                setNameDraft(c.tailscale.name ?? "")
                setLoginsDraft(c.tailscale.allowedLogins.join(", "))
            })
            .catch(() => setConfig(null))
        void resolveTailscaleName()
            .then(setResolvedName)
            .catch(() => setResolvedName(null))
    }, [])

    if (!config) {
        return (
            <section className="settings-section">
                <h2>Web UI</h2>
                <p className="settings-empty">Loading…</p>
            </section>
        )
    }

    const ts = config.tailscale

    const toggle = async () => {
        const next = !config.enabled
        setConfig({ ...config, enabled: next })
        try {
            await setWebEnabled(next)
        } catch (err) {
            setConfig({ ...config, enabled: !next })
            console.warn("failed to update web-enabled", err)
        }
    }

    const changePort = async (port: number) => {
        if (!Number.isFinite(port) || port < 1 || port > 65535) return
        const previous = config.port
        setConfig({ ...config, port })
        try {
            await setWebPort(port)
        } catch (err) {
            setConfig({ ...config, port: previous })
            console.warn("failed to update web port", err)
        }
    }

    const toggleTailscale = async () => {
        const next = !ts.enabled
        setConfig({ ...config, tailscale: { ...ts, enabled: next } })
        try {
            await setWebTailscaleEnabled(next)
            if (next) setResolvedName(await resolveTailscaleName().catch(() => null))
        } catch (err) {
            setConfig({ ...config, tailscale: { ...ts, enabled: !next } })
            console.warn("failed to update tailscale-enabled", err)
        }
    }

    const commitName = async () => {
        const next = nameDraft.trim() || null
        if ((ts.name ?? "") === (next ?? "")) return
        try {
            await setWebTailscaleName(next)
            setConfig({ ...config, tailscale: { ...ts, name: next } })
            setResolvedName(await resolveTailscaleName().catch(() => null))
        } catch (err) {
            setNameDraft(ts.name ?? "")
            console.warn("failed to set tailscale name", err)
        }
    }

    const commitLogins = async () => {
        const next = loginsDraft
            .split(",")
            .map((s) => s.trim())
            .filter((s) => s.length > 0)
        try {
            await setWebTailscaleAllowedLogins(next)
            setConfig({ ...config, tailscale: { ...ts, allowedLogins: next } })
            setLoginsDraft(next.join(", "))
        } catch (err) {
            setLoginsDraft(ts.allowedLogins.join(", "))
            console.warn("failed to set tailscale logins", err)
        }
    }

    return (
        <section className="settings-section">
            <h2>Web UI</h2>
            <p className="settings-help">
                Also serve SpecForge in a browser at{" "}
                <code>http://127.0.0.1:{config.port}</code>. The tab mirrors this
                app's live state. Loopback only — never exposed on your network.
                Restart SpecForge to apply changes.
            </p>
            <label className="settings-toggle-row">
                <input
                    type="checkbox"
                    checked={config.enabled}
                    onChange={toggle}
                />
                <span>Serve the web UI in a browser</span>
            </label>
            <label className="settings-field">
                <span className="settings-field-label">Port</span>
                <input
                    className="settings-text-input"
                    type="number"
                    min={1}
                    max={65535}
                    step={1}
                    value={config.port}
                    onChange={(e) => void changePort(parseInt(e.target.value, 10))}
                    aria-label="Web UI port"
                />
            </label>

            {config.enabled && (
                <>
                    <p className="settings-help">
                        <strong>Reach it from another device.</strong> The server
                        listens only on this machine, so forward the port over
                        SSH — the tunnel presents it as <code>localhost</code> on
                        the other device, which is exactly what a loopback-only
                        server accepts (no extra exposure). Then open{" "}
                        <code>http://localhost:{config.port}</code> there.
                    </p>
                    <p className="settings-help">
                        Over SSH:{" "}
                        <code>
                            ssh -N -L {config.port}:localhost:{config.port}{" "}
                            you@this-machine
                        </code>
                    </p>
                    <p className="settings-help">
                        Over Tailscale — the same tunnel, addressed by your
                        machine's tailnet name:{" "}
                        <code>
                            ssh -N -L {config.port}:localhost:{config.port}{" "}
                            you@your-machine.tailnet.ts.net
                        </code>
                    </p>

                    <p className="settings-help">
                        <strong>Or reach it directly over Tailscale.</strong>{" "}
                        Enabling this trusts your machine's own tailnet name in the
                        access check so <code>tailscale serve</code> can proxy to
                        the (still loopback-bound) server — no SSH tunnel. The
                        server is never bound to a non-loopback interface.
                    </p>
                    <label className="settings-toggle-row">
                        <input
                            type="checkbox"
                            checked={ts.enabled}
                            onChange={toggleTailscale}
                        />
                        <span>Allow access via Tailscale Serve</span>
                    </label>

                    {ts.enabled && (
                        <>
                            <p className="settings-help">
                                Trusted tailnet name:{" "}
                                {resolvedName ? (
                                    <code>{resolvedName}</code>
                                ) : (
                                    <em>
                                        not detected — is Tailscale running? Set it
                                        manually below.
                                    </em>
                                )}
                            </p>
                            <p className="settings-help">
                                Run <code>tailscale serve --bg {config.port}</code>,
                                then open{" "}
                                <code>
                                    https://
                                    {resolvedName ??
                                        "your-machine.tailnet.ts.net"}
                                    /
                                </code>{" "}
                                from any device on your tailnet.
                            </p>
                            <label className="settings-field">
                                <span className="settings-field-label">
                                    Tailnet name override (optional)
                                </span>
                                <input
                                    className="settings-text-input"
                                    value={nameDraft}
                                    placeholder={resolvedName ?? "auto-detected"}
                                    onChange={(e) => setNameDraft(e.target.value)}
                                    onBlur={() => void commitName()}
                                    onKeyDown={(e) => {
                                        if (e.key === "Enter")
                                            (e.target as HTMLInputElement).blur()
                                    }}
                                    aria-label="Tailnet name override"
                                />
                            </label>
                            <label className="settings-field">
                                <span className="settings-field-label">
                                    Restrict to logins (optional, comma-separated)
                                </span>
                                <input
                                    className="settings-text-input"
                                    value={loginsDraft}
                                    placeholder="alice@example.com, bob@example.com"
                                    onChange={(e) =>
                                        setLoginsDraft(e.target.value)
                                    }
                                    onBlur={() => void commitLogins()}
                                    onKeyDown={(e) => {
                                        if (e.key === "Enter")
                                            (e.target as HTMLInputElement).blur()
                                    }}
                                    aria-label="Allowed Tailscale logins"
                                />
                            </label>
                        </>
                    )}
                </>
            )}
        </section>
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
    /// The other registered folders sharing this row's presentation key —
    /// sibling worktrees of one repository. Non-empty means this row's switch,
    /// name and tint are shared with them, which the row has to say out loud
    /// (they are stored per repository, not per folder).
    siblings: RegisteredWorkspace[]
    onRemove: () => void
}

function WorkspaceRow({ ws, siblings, onRemove }: WorkspaceRowProps) {
    // Local copy of the rename input so editing is responsive; commit on
    // blur and Enter. Cleared input becomes `null` server-side so the row
    // reverts to its basename-derived default.
    const [draftName, setDraftName] = useState<string>(ws.displayName ?? "")

    // Why this row's last write didn't take — rename, tint or park/un-park.
    // Row-scoped (not lifted to the section like `addError`) so with several
    // rows listed the message sits on the row the user actually operated, and
    // one element serves all three of its controls: only one is ever operated
    // at a time, and each message names the control it came from. Every
    // per-workspace control has to report here, not to `console.warn` —
    // `workspace-registry`'s *Settings View* requirement, and the desktop has
    // no console the user can see, so a rejected write would otherwise be
    // indistinguishable from a control that silently does nothing.
    const [rowError, setRowError] = useState<string | null>(null)

    // Escape abandons the edit by resetting state and blurring — but blur()
    // dispatches synchronously, before the reset has flushed, so the blur
    // handler's closure still sees the abandoned draft. This flag tells the
    // commit-on-blur path to stand down for that one blur.
    const abandoningRef = useRef(false)

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
        setRowError(null)
        try {
            await setWorkspacePresentation(
                ws.uri,
                ws.repoId,
                next.length === 0 ? null : next,
                ws.color,
            )
        } catch (err) {
            // Snap back to the persisted value on failure, and say why: the
            // field must show the STORED name, never the attempted one, and a
            // field that silently reverts what was just typed is the exact
            // "does nothing" the requirement rules out.
            setDraftName(ws.displayName ?? "")
            setRowError(`Couldn't rename this workspace — ${prettifyError(err)}`)
        }
    }

    // Parking a row hides it from the tree, the tray badge and notifications
    // while leaving every Dashboard figure — and the registration itself —
    // untouched. The backend emits `workspace-presentation-updated`, which the
    // workspaces hook already turns into a full refresh, so there is nothing to
    // refetch here.
    const toggleDisabled = async () => {
        setRowError(null)
        try {
            await setWorkspaceDisabled(ws.uri, ws.repoId, !ws.disabled)
        } catch (err) {
            // Nothing else reports this. The switch is driven by props, which
            // only move once the backend's presentation-updated event lands —
            // so a failed write leaves it visually unchanged and, without this,
            // entirely silent in an app with no visible console.
            setRowError(
                `Couldn't ${ws.disabled ? "enable" : "disable"} this workspace — ${prettifyError(err)}`,
            )
        }
    }

    // The message survives only while it is still true. Once the stored value
    // the control shows actually moves — this row's next successful write, or a
    // sibling row's, since one flag, name and tint serve the whole repository —
    // the failure it described is no longer the last word, and leaving it
    // beside a control that has since moved would be its own lie.
    useEffect(() => {
        setRowError(null)
    }, [ws.disabled, ws.displayName, ws.color])

    const setColor = async (color: PaletteColor | null) => {
        if (color === ws.color) return
        setRowError(null)
        try {
            await setWorkspacePresentation(
                ws.uri,
                ws.repoId,
                ws.displayName,
                color,
            )
        } catch (err) {
            // The swatches render `ws.color`, so a rejected write leaves the
            // STORED tint selected and the click looks like it did nothing.
            setRowError(`Couldn't set this workspace's colour — ${prettifyError(err)}`)
        }
    }

    // The switch writes one flag per REPOSITORY, so on a row with siblings it
    // moves theirs too. Announced on the control itself (not only shown in the
    // row's note) so the shared scope reaches screen readers as well.
    const sharedScopeSuffix =
        siblings.length > 0
            ? ` — shared with ${siblings.length} sibling worktree${
                  siblings.length === 1 ? "" : "s"
              } of the same repository`
            : ""

    // Radio-group keyboard contract for the palette: the checked swatch is
    // the group's single tab stop, and arrows move-and-select with wrap —
    // what role="radio" promises assistive tech.
    const paletteValues: (PaletteColor | null)[] = [null, ...PALETTE_COLORS]
    const handlePaletteKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
        const dir =
            e.key === "ArrowRight" || e.key === "ArrowDown"
                ? 1
                : e.key === "ArrowLeft" || e.key === "ArrowUp"
                  ? -1
                  : 0
        if (dir === 0) return
        e.preventDefault()
        const current = paletteValues.indexOf(ws.color)
        const nextIndex =
            (current + dir + paletteValues.length) % paletteValues.length
        void setColor(paletteValues[nextIndex] ?? null)
        e.currentTarget
            .querySelectorAll<HTMLElement>('[role="radio"]')
            [nextIndex]?.focus()
    }

    return (
        <li
            className={`workspace-row${ws.isMissing ? " missing" : ""}${
                ws.disabled ? " workspace-row--disabled" : ""
            }`}
        >
            <div className="workspace-info">
                <div className="workspace-name">
                    <input
                        className="workspace-name-input"
                        value={draftName}
                        placeholder={ws.name}
                        onChange={(e) => setDraftName(e.target.value)}
                        onBlur={() => {
                            if (abandoningRef.current) {
                                abandoningRef.current = false
                                return
                            }
                            void commitName()
                        }}
                        onKeyDown={(e) => {
                            if (e.key === "Enter") {
                                e.preventDefault()
                                ;(e.target as HTMLInputElement).blur()
                            } else if (e.key === "Escape") {
                                // This input consumes Escape (abandon the
                                // edit) — keep it from reaching the app-level
                                // close-Settings fallback.
                                e.stopPropagation()
                                abandoningRef.current = true
                                setDraftName(ws.displayName ?? "")
                                ;(e.target as HTMLInputElement).blur()
                            }
                        }}
                        aria-label={`Display name for ${ws.name}`}
                    />
                    {ws.isMissing && (
                        <span className="chip chip--warn">missing</span>
                    )}
                    {ws.disabled && <span className="chip">disabled</span>}
                </div>
                <div className="workspace-path" title={ws.uri}>
                    {ws.uri}
                </div>
                {siblings.length > 0 && (
                    <div
                        className="workspace-shared-note"
                        title={siblings.map((s) => s.uri).join("\n")}
                    >
                        Same repository as {siblings.length} other registered
                        folder{siblings.length === 1 ? "" : "s"} — they share
                        this switch, name and tint.
                    </div>
                )}
                <div
                    className="workspace-palette"
                    role="radiogroup"
                    aria-label="Workspace tint colour"
                    onKeyDown={handlePaletteKeyDown}
                >
                    <button
                        type="button"
                        className={`palette-swatch palette-swatch--none${
                            ws.color === null ? " selected" : ""
                        }`}
                        onClick={() => void setColor(null)}
                        role="radio"
                        aria-label="No tint"
                        aria-checked={ws.color === null}
                        tabIndex={ws.color === null ? 0 : -1}
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
                            role="radio"
                            aria-label={`Tint colour ${token}`}
                            aria-checked={ws.color === token}
                            tabIndex={ws.color === token ? 0 : -1}
                            title={token}
                        />
                    ))}
                </div>
                {rowError && (
                    <p className="settings-error" role="alert">
                        {rowError}
                    </p>
                )}
            </div>
            <div className="workspace-actions">
                <button
                    type="button"
                    className="workspace-toggle"
                    role="switch"
                    aria-checked={!ws.disabled}
                    aria-label={`Enable ${ws.name}${sharedScopeSuffix}`}
                    title={
                        (ws.disabled
                            ? "Disabled — hidden from the tree, tray badge and notifications. Dashboard totals still include it."
                            : "Enabled") + sharedScopeSuffix
                    }
                    onClick={() => void toggleDisabled()}
                >
                    <span className="workspace-toggle-knob" aria-hidden="true" />
                </button>
                <button
                    className="btn-remove"
                    onClick={onRemove}
                    aria-label={`Remove ${ws.name}`}
                >
                    Remove
                </button>
            </div>
        </li>
    )
}
