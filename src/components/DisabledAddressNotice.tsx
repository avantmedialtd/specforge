import { useState } from "react"
import { setWorkspaceDisabled } from "../api"
import { rowLabel } from "../workspaceRows"
import type { RegisteredWorkspace } from "../types"
import { EmptyState } from "./EmptyState"
import { prettifyError } from "./errors"

interface DisabledAddressNoticeProps {
    /// The parked top-level row(s) the address names — more than one only when
    /// two parked rows share a slug, in which case each gets its own control.
    workspaces: RegisteredWorkspace[]
    onOpenSettings: () => void
}

/// What an address into a DISABLED workspace lands on. Distinct from the
/// not-found notice on purpose: a parked workspace is still registered, and
/// saying "this doesn't match anything registered" would contradict the one
/// promise the feature makes — that disabling is reversible and loses nothing
/// (`view-routing`: *Cold-Load Address Resolution*).
///
/// Re-enabling needs no follow-up navigation: the command emits
/// `workspace-presentation-updated`, `useWorkspaces` refreshes, the row returns
/// to `views`, and the unchanged address resolves on the next render.
export function DisabledAddressNotice({
    workspaces,
    onOpenSettings,
}: DisabledAddressNoticeProps) {
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string | null>(null)

    const enable = async (ws: RegisteredWorkspace) => {
        setError(null)
        setBusy(true)
        try {
            await setWorkspaceDisabled(ws.uri, ws.repoId, false)
        } catch (err) {
            setError(prettifyError(err))
        } finally {
            setBusy(false)
        }
    }

    const only = workspaces.length === 1 ? workspaces[0] : null

    return (
        <EmptyState
            title="This workspace is disabled"
            body={
                <>
                    <p>
                        {only
                            ? `${rowLabel(only)} is still registered, but it's disabled — it's hidden from the tree, so this link has nothing to open.`
                            : "This link names a workspace that is still registered but disabled, so it's hidden from the tree and has nothing to open."}
                    </p>
                    <div className="disabled-notice-actions">
                        {workspaces.map((ws) => (
                            <button
                                key={ws.uri}
                                className="archive-back"
                                disabled={busy}
                                onClick={() => void enable(ws)}
                            >
                                Enable {rowLabel(ws)}
                            </button>
                        ))}
                        <button className="archive-back" onClick={onOpenSettings}>
                            Open Settings
                        </button>
                    </div>
                    {error && (
                        <p className="settings-error" role="alert">
                            {error}
                        </p>
                    )}
                </>
            }
        />
    )
}
