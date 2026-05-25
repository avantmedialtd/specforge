import { useEffect, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import {
    getLaunchOnLogin,
    getNotificationsEnabled,
    registerWorkspace,
    setLaunchOnLogin,
    setNotificationsEnabled,
    unregisterWorkspace,
} from "../api"
import { Close } from "./icons"
import type { RegisteredWorkspace } from "../types"

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
                            <li
                                key={ws.uri}
                                className={`workspace-row${ws.isMissing ? " missing" : ""}`}
                            >
                                <div className="workspace-info">
                                    <div className="workspace-name">
                                        {ws.name}
                                        {ws.isMissing && (
                                            <span className="chip chip--warn">
                                                missing
                                            </span>
                                        )}
                                    </div>
                                    <div className="workspace-path" title={ws.uri}>
                                        {ws.uri}
                                    </div>
                                </div>
                                <button
                                    className="btn-remove"
                                    onClick={() => handleRemove(ws.uri)}
                                    aria-label={`Remove ${ws.name}`}
                                >
                                    Remove
                                </button>
                            </li>
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

function prettifyError(err: unknown): string {
    const s = String(err)
    // Tauri wraps command errors in `IpcMessage:Error("...")` shells —
    // unwrap to the readable inner message where possible.
    const inner = s.match(/Error\("(.+)"\)/)?.[1] ?? s
    return inner.replace(/^[A-Za-z]+::/, "")
}
