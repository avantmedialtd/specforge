import { useEffect, useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import {
    getWslPollIntervalSecs,
    isTauri,
    registerWorkspace,
    setWorkspaceDisabled,
    setWorkspacePresentation,
    setWslPollIntervalSecs,
    unregisterWorkspace,
} from "../../api"
import { parsePollSeconds } from "../../settingsFields"
import { PALETTE_COLORS, type PaletteColor, type RegisteredWorkspace } from "../../types"
import { siblingsOf } from "../../workspaceRows"
import { prettifyError } from "../errors"
import { CommittedField } from "./CommittedField"
import { SettingsRow } from "./SettingsRow"
import { Switch } from "./Switch"

interface WorkspacesGroupProps {
    workspaces: RegisteredWorkspace[]
    onWorkspacesChanged: () => Promise<void>
}

/// Settings → Workspaces: the registered folders, adding one, and — on
/// Windows only — how often WSL-hosted ones are re-scanned
/// (`workspace-registry`: *Settings View*).
export function WorkspacesGroup({ workspaces, onWorkspacesChanged }: WorkspacesGroupProps) {
    const [addError, setAddError] = useState<string | null>(null)
    const [busy, setBusy] = useState(false)
    // In the browser there is no native folder dialog, so the user types a path.
    const [pathInput, setPathInput] = useState("")
    // `null` until read, and for good where WSL cannot occur: the backing query
    // reports the setting is not applicable, and the sub-section is then
    // omitted outright rather than left empty (`settings-view`: *Groups With
    // Nothing to Offer Are Omitted*).
    const [wslPollSecs, setWslPollSecs] = useState<number | null>(null)

    useEffect(() => {
        let cancelled = false
        getWslPollIntervalSecs()
            .then((secs) => {
                if (!cancelled) setWslPollSecs(secs)
            })
            .catch(() => {})
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
                title: "Choose an OpenSpec workspace or git repository folder",
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
    // (validated server-side by the same rule as the desktop picker: an
    // `openspec/` directory, or a folder inside a git working tree).
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

    return (
        <>
            <p className="settings-help">
                OpenSpec workspaces (folders containing an <code>openspec/</code> directory) and git
                repositories. Add each workspace whose specs you want to monitor, and any companion
                repository whose files and pull requests you want beside them.
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
                            onWorkspacesChanged={onWorkspacesChanged}
                        />
                    ))}
                </ul>
            )}
            {isTauri() ? (
                <button className="btn-primary" onClick={handleAdd} disabled={busy}>
                    {busy ? "Adding…" : "+ Add workspace"}
                </button>
            ) : (
                <div className="identity-add-form">
                    <input
                        className="settings-text-input"
                        value={pathInput}
                        placeholder="/path/to/an/openspec/workspace/or/git/repository"
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

            {wslPollSecs != null && (
                <section className="settings-section settings-section--spaced">
                    <h3 className="settings-subheading">WSL workspaces</h3>
                    <SettingsRow
                        layout="stacked"
                        title="Poll interval (seconds)"
                        controlId="settings-wsl-poll"
                        description={
                            <>
                                Workspaces stored in the WSL filesystem (
                                <code>\\wsl.localhost\…</code>) are watched by polling, because
                                Windows receives no change events across the share. How often to
                                re-scan.
                            </>
                        }
                        control={
                            <CommittedField
                                id="settings-wsl-poll"
                                stored={wslPollSecs}
                                format={String}
                                parse={parsePollSeconds}
                                numeric
                                onCommit={async (secs) => {
                                    await setWslPollIntervalSecs(secs)
                                    setWslPollSecs(secs)
                                }}
                            />
                        }
                    />
                </section>
            )}
        </>
    )
}

interface WorkspaceRowProps {
    ws: RegisteredWorkspace
    /// The other registered folders sharing this row's presentation key —
    /// sibling worktrees of one repository. Non-empty means this row's switch,
    /// name and tint are shared with them, which the row has to say out loud
    /// (they are stored per repository, not per folder).
    siblings: RegisteredWorkspace[]
    onWorkspacesChanged: () => Promise<void>
}

function WorkspaceRow({ ws, siblings, onWorkspacesChanged }: WorkspaceRowProps) {
    // Local copy of the rename input so editing is responsive; commit on
    // blur and Enter. Cleared input becomes `null` server-side so the row
    // reverts to its basename-derived default.
    const [draftName, setDraftName] = useState<string>(ws.displayName ?? "")

    // Why this row's last write didn't take — rename, tint, park/un-park or
    // removal. Row-scoped (not lifted to the group like `addError`) so with
    // several rows listed the message sits on the row the user actually
    // operated, and one element serves all of its controls: only one is ever
    // operated at a time, and each message names the control it came from.
    // Every per-workspace control has to report here, not to `console.warn` —
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
    // refresh path updated it), pull the new value in.
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

    const remove = async () => {
        setRowError(null)
        try {
            await unregisterWorkspace(ws.uri)
            await onWorkspacesChanged()
        } catch (err) {
            setRowError(`Couldn't remove this workspace — ${prettifyError(err)}`)
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
            await setWorkspacePresentation(ws.uri, ws.repoId, ws.displayName, color)
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
        const nextIndex = (current + dir + paletteValues.length) % paletteValues.length
        void setColor(paletteValues[nextIndex] ?? null)
        e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[nextIndex]?.focus()
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
                    {ws.isMissing && <span className="chip chip--warn">missing</span>}
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
                        Same repository as {siblings.length} other registered folder
                        {siblings.length === 1 ? "" : "s"} — they share this switch, name and tint.
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
                {/* The settings switch, named by its aria-label: this row has no
                    title for a <label> to hang on, so the label carries the
                    workspace and its shared scope (`settings-view`: Settings
                    Rows). Driven by the STORED flag — a write that fails leaves
                    it where it was, and the row says why. */}
                <Switch
                    checked={!ws.disabled}
                    onChange={() => void toggleDisabled()}
                    ariaLabel={`Enable ${ws.name}${sharedScopeSuffix}`}
                    title={
                        (ws.disabled
                            ? "Disabled — hidden from the tree, tray badge and notifications. Dashboard totals still include it."
                            : "Enabled") + sharedScopeSuffix
                    }
                />
                <button className="btn-remove" onClick={() => void remove()} aria-label={`Remove ${ws.name}`}>
                    Remove
                </button>
            </div>
        </li>
    )
}
