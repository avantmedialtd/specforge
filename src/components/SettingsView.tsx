import { useEffect, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import {
    getLaunchOnLogin,
    getNotificationsEnabled,
    registerWorkspace,
    setLaunchOnLogin,
    setNotificationsEnabled,
    setWorkspacePresentation,
    unregisterWorkspace,
} from "../api"
import { Close } from "./icons"
import { PALETTE_COLORS, type PaletteColor, type RegisteredWorkspace } from "../types"

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
    const [addError, setAddError] = useState<string | null>(null)
    const [busy, setBusy] = useState(false)

    useEffect(() => {
        let cancelled = false
        Promise.all([
            getLaunchOnLogin().catch(() => false),
            getNotificationsEnabled().catch(() => true),
        ]).then(([launch, notif]) => {
            if (cancelled) return
            setLaunch(launch)
            setNotifs(notif)
        })
        return () => {
            cancelled = true
        }
    }, [])

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
