import { useEffect, useRef } from "react"
import { isTauri } from "../../api"
import {
    SETTINGS_GROUP_LABELS,
    effectiveSettingsGroup,
    groupsForHost,
    type SettingsGroup,
} from "../../settingsGroups"
import type { DocumentWidth, RegisteredWorkspace } from "../../types"
import { Close } from "../icons"
import { DesktopGroup } from "./DesktopGroup"
import { IdentityGroup } from "./IdentityGroup"
import { IntegrationsGroup } from "./IntegrationsGroup"
import { LayoutGroup } from "./LayoutGroup"
import { SettingsNav } from "./SettingsNav"
import { WorkspacesGroup } from "./WorkspacesGroup"

interface SettingsViewProps {
    /// The group the address names.
    group: SettingsGroup
    /// Moving between groups. `App` routes it through `go()`, whose overlay
    /// rule replaces the history entry — the groups are parts of one transient
    /// view, so Back from any of them closes Settings (`view-routing`:
    /// *History Entry Discipline*).
    onSelectGroup: (group: SettingsGroup) => void
    workspaces: RegisteredWorkspace[]
    onWorkspacesChanged: () => Promise<void>
    onClose: () => void
    /// The reading width, owned by `App` so the reconciliation and the
    /// cross-window listener exist whether or not Settings is open.
    documentWidth: DocumentWidth
    onDocumentWidthChange: (width: DocumentWidth) => Promise<void>
    /// The Commit history switch, owned by `App` for the same reason: it
    /// decides whether the main window has a rail whether or not Settings is
    /// open, and turning it on also shows the rail on this surface.
    commitHistoryEnabled: boolean
    onCommitHistoryEnabledChange: (enabled: boolean) => Promise<void>
}

/// The Settings view (`settings-view`): five groups, one shown at a time in the
/// center pane, where the tree and the rail stay beside it — so a renamed or
/// tinted workspace, or a panel moved between slots, updates in plain view.
export function SettingsView({
    group,
    onSelectGroup,
    workspaces,
    onWorkspacesChanged,
    onClose,
    documentWidth,
    onDocumentWidthChange,
    commitHistoryEnabled,
    onCommitHistoryEnabledChange,
}: SettingsViewProps) {
    const host = { desktop: isTauri() }
    const groups = groupsForHost(host)
    // What the address names may be a group this host omits; `App` replaces
    // such an address with the default group's, and until that lands the view
    // shows the default rather than an omitted group's contents.
    const shown = effectiveSettingsGroup(group, host)

    // Each group starts at its top: the view is the scroll container, and a
    // new group shown at the previous one's scroll offset would open mid-way.
    const viewRef = useRef<HTMLDivElement>(null)
    useEffect(() => {
        viewRef.current?.scrollTo({ top: 0 })
    }, [shown])

    return (
        <div className="settings-view" ref={viewRef}>
            <div className="settings-view-inner">
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
                <div className="settings-body">
                    <SettingsNav groups={groups} current={shown} onSelect={onSelectGroup} />
                    <section className="settings-group" aria-labelledby="settings-group-heading">
                        <h2 className="settings-group-heading" id="settings-group-heading">
                            {SETTINGS_GROUP_LABELS[shown]}
                        </h2>
                        {shown === "workspaces" && (
                            <WorkspacesGroup
                                workspaces={workspaces}
                                onWorkspacesChanged={onWorkspacesChanged}
                            />
                        )}
                        {shown === "layout" && (
                            <LayoutGroup
                                documentWidth={documentWidth}
                                onDocumentWidthChange={onDocumentWidthChange}
                                commitHistoryEnabled={commitHistoryEnabled}
                                onCommitHistoryEnabledChange={onCommitHistoryEnabledChange}
                                onSelectGroup={onSelectGroup}
                            />
                        )}
                        {shown === "integrations" && <IntegrationsGroup onSelectGroup={onSelectGroup} />}
                        {shown === "identity" && <IdentityGroup />}
                        {shown === "desktop" && <DesktopGroup />}
                    </section>
                </div>
            </div>
        </div>
    )
}
