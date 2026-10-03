import { useEffect, useState } from "react"
import {
    getLaunchOnLogin,
    getNotificationsEnabled,
    getWebConfig,
    resolveTailscaleName,
    setLaunchOnLogin,
    setNotificationsEnabled,
    setWebEnabled,
    setWebPort,
    setWebTailscaleAllowedLogins,
    setWebTailscaleEnabled,
    setWebTailscaleName,
} from "../../api"
import { parseList, parsePort, parseText, sameList } from "../../settingsFields"
import type { WebServerConfig } from "../../types"
import { prettifyError } from "../errors"
import { CommittedField } from "./CommittedField"
import { SettingsRow } from "./SettingsRow"
import { Switch } from "./Switch"
import { useSettingSwitch } from "./useSettingSwitch"

/// Settings → Desktop app: exactly the settings only the desktop shell can
/// honour — launch at login lives in the OS, notifications are the OS's, and
/// the embedded web server is a native process. The browser skin therefore
/// omits the whole group (`web-ui`: *Desktop-Only Settings Are Hidden in the
/// Web UI*); `SettingsView` never mounts it there.
export function DesktopGroup() {
    const launch = useSettingSwitch(getLaunchOnLogin, setLaunchOnLogin, false)
    const notifications = useSettingSwitch(getNotificationsEnabled, setNotificationsEnabled, true)

    return (
        <>
            <section className="settings-section">
                <SettingsRow
                    title="Launch at login"
                    controlId="settings-launch-at-login"
                    description="Start SpecForge when you log in to this computer."
                    error={launch.error}
                    control={
                        <Switch
                            id="settings-launch-at-login"
                            checked={launch.value ?? false}
                            disabled={launch.value === null}
                            onChange={(next) => void launch.flip(next)}
                        />
                    }
                />
                <SettingsRow
                    title="Notifications"
                    controlId="settings-notifications"
                    description="Show a system notification when a change is added or archived."
                    error={notifications.error}
                    control={
                        <Switch
                            id="settings-notifications"
                            checked={notifications.value ?? false}
                            disabled={notifications.value === null}
                            onChange={(next) => void notifications.flip(next)}
                        />
                    }
                />
            </section>
            <WebAccessSection />
        </>
    )
}

/// The embedded local web server: when on, the desktop app also serves the
/// browser skin on a loopback port from the same live state. Changes take
/// effect at the next launch.
function WebAccessSection() {
    const [config, setConfig] = useState<WebServerConfig | null>(null)
    const [loadFailed, setLoadFailed] = useState(false)
    // The tailnet name the server would currently trust (manual or discovered).
    const [resolvedName, setResolvedName] = useState<string | null>(null)
    const [serveError, setServeError] = useState<string | null>(null)
    const [tailscaleError, setTailscaleError] = useState<string | null>(null)

    useEffect(() => {
        let cancelled = false
        getWebConfig()
            .then((c) => {
                if (!cancelled) setConfig(c)
            })
            .catch(() => {
                if (!cancelled) setLoadFailed(true)
            })
        resolveTailscaleName()
            .then((name) => {
                if (!cancelled) setResolvedName(name)
            })
            .catch(() => {
                if (!cancelled) setResolvedName(null)
            })
        return () => {
            cancelled = true
        }
    }, [])

    if (!config) {
        return (
            <section className="settings-section">
                <h3 className="settings-subheading">Web access</h3>
                <p className="settings-empty">
                    {loadFailed ? "Could not load the web server settings." : "Loading…"}
                </p>
            </section>
        )
    }

    const ts = config.tailscale

    const toggleServe = async (next: boolean) => {
        setServeError(null)
        setConfig((c) => (c ? { ...c, enabled: next } : c))
        try {
            await setWebEnabled(next)
        } catch (err) {
            setConfig((c) => (c ? { ...c, enabled: !next } : c))
            setServeError(`Couldn't save this setting — ${prettifyError(err)}`)
        }
    }

    const toggleTailscale = async (next: boolean) => {
        setTailscaleError(null)
        setConfig((c) => (c ? { ...c, tailscale: { ...c.tailscale, enabled: next } } : c))
        try {
            await setWebTailscaleEnabled(next)
            if (next) setResolvedName(await resolveTailscaleName().catch(() => null))
        } catch (err) {
            setConfig((c) => (c ? { ...c, tailscale: { ...c.tailscale, enabled: !next } } : c))
            setTailscaleError(`Couldn't save this setting — ${prettifyError(err)}`)
        }
    }

    return (
        <section className="settings-section">
            <h3 className="settings-subheading">Web access</h3>
            <SettingsRow
                title="Serve the web UI in a browser"
                controlId="settings-web-enabled"
                description={
                    <>
                        Also serve SpecForge at <code>http://127.0.0.1:{config.port}</code>. The tab
                        mirrors this app's live state. Loopback only — never exposed on your
                        network. Restart SpecForge to apply changes.
                    </>
                }
                error={serveError}
                control={
                    <Switch
                        id="settings-web-enabled"
                        checked={config.enabled}
                        onChange={(next) => void toggleServe(next)}
                    />
                }
            />
            <SettingsRow
                layout="stacked"
                title="Port"
                controlId="settings-web-port"
                control={
                    <CommittedField
                        id="settings-web-port"
                        stored={config.port}
                        format={String}
                        parse={parsePort}
                        numeric
                        onCommit={async (port) => {
                            await setWebPort(port)
                            setConfig((c) => (c ? { ...c, port } : c))
                        }}
                    />
                }
            />

            {config.enabled && (
                <>
                    <details className="settings-disclosure">
                        <summary>Reach it from another device</summary>
                        <p className="settings-help">
                            The server listens only on this machine, so forward the port over SSH —
                            the tunnel presents it as <code>localhost</code> on the other device,
                            which is exactly what a loopback-only server accepts (no extra exposure).
                            Then open <code>http://localhost:{config.port}</code> there.
                        </p>
                        <p className="settings-help">
                            Over SSH:{" "}
                            <code>
                                ssh -N -L {config.port}:localhost:{config.port} you@this-machine
                            </code>
                        </p>
                        <p className="settings-help">
                            Over Tailscale — the same tunnel, addressed by your machine's tailnet
                            name:{" "}
                            <code>
                                ssh -N -L {config.port}:localhost:{config.port}{" "}
                                you@your-machine.tailnet.ts.net
                            </code>
                        </p>
                    </details>

                    <SettingsRow
                        title="Allow access via Tailscale Serve"
                        controlId="settings-web-tailscale"
                        description={
                            <>
                                Trusts your machine's own tailnet name in the access check, so{" "}
                                <code>tailscale serve</code> can proxy to the (still loopback-bound)
                                server — no SSH tunnel. The server is never bound to a non-loopback
                                interface.
                            </>
                        }
                        error={tailscaleError}
                        control={
                            <Switch
                                id="settings-web-tailscale"
                                checked={ts.enabled}
                                onChange={(next) => void toggleTailscale(next)}
                            />
                        }
                    />

                    {ts.enabled && (
                        <>
                            <p className="settings-help">
                                Trusted tailnet name:{" "}
                                {resolvedName ? (
                                    <code>{resolvedName}</code>
                                ) : (
                                    <em>not detected — is Tailscale running? Set it manually below.</em>
                                )}
                            </p>
                            <p className="settings-help">
                                Run <code>tailscale serve --bg {config.port}</code>, then open{" "}
                                <code>https://{resolvedName ?? "your-machine.tailnet.ts.net"}/</code>{" "}
                                from any device on your tailnet.
                            </p>
                            <SettingsRow
                                layout="stacked"
                                title="Tailnet name override (optional)"
                                controlId="settings-web-tailnet-name"
                                control={
                                    <CommittedField
                                        id="settings-web-tailnet-name"
                                        stored={ts.name ?? null}
                                        format={(name) => name ?? ""}
                                        parse={parseText}
                                        placeholder={resolvedName ?? "auto-detected"}
                                        onCommit={async (name) => {
                                            await setWebTailscaleName(name)
                                            setConfig((c) =>
                                                c ? { ...c, tailscale: { ...c.tailscale, name } } : c,
                                            )
                                            setResolvedName(await resolveTailscaleName().catch(() => null))
                                        }}
                                    />
                                }
                            />
                            <SettingsRow
                                layout="stacked"
                                title="Restrict to logins (optional, comma-separated)"
                                controlId="settings-web-tailnet-logins"
                                control={
                                    <CommittedField
                                        id="settings-web-tailnet-logins"
                                        stored={ts.allowedLogins}
                                        format={(logins) => logins.join(", ")}
                                        parse={parseList}
                                        equals={sameList}
                                        placeholder="alice@example.com, bob@example.com"
                                        onCommit={async (allowedLogins) => {
                                            await setWebTailscaleAllowedLogins(allowedLogins)
                                            setConfig((c) =>
                                                c
                                                    ? { ...c, tailscale: { ...c.tailscale, allowedLogins } }
                                                    : c,
                                            )
                                        }}
                                    />
                                }
                            />
                        </>
                    )}
                </>
            )}
        </section>
    )
}
