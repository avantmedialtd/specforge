/// The Settings view's groups: which exist, in what order, under what label,
/// and which of them a host offers (`settings-view`: *Settings Are Organised
/// Into Groups*, *Groups With Nothing to Offer Are Omitted*).
///
/// Kept out of the component tree because the routing codec needs the group
/// ids and `src/routing/` imports no components. Kept pure for the reason
/// `docWidth.ts` gives in its header: this repository has no component tests,
/// and a `src/`-only diff short-circuits the mutation gate, so this module and
/// its tests are the grouping's only automated coverage.

/// Every group, in the order the navigation offers them. The ids double as the
/// URL path segment after `/settings`, so renaming one breaks saved links.
export const SETTINGS_GROUPS = ["workspaces", "layout", "integrations", "identity", "desktop"] as const

export type SettingsGroup = (typeof SETTINGS_GROUPS)[number]

/// The group Settings opens at when nothing names one — the bare `/settings`,
/// the sidebar row, the macOS menu item — and the one an address naming a
/// group this host omits falls back to.
export const DEFAULT_SETTINGS_GROUP: SettingsGroup = "workspaces"

/// The heading each group is shown under, which is also its navigation entry.
export const SETTINGS_GROUP_LABELS: Record<SettingsGroup, string> = {
    workspaces: "Workspaces",
    layout: "Layout",
    integrations: "Integrations",
    identity: "Identity",
    desktop: "Desktop app",
}

/// What the current host can honour. Only the desktop shell can launch at
/// login, raise OS notifications or serve the web UI, so `desktop` alone
/// decides whether the Desktop app group exists.
export interface SettingsHost {
    desktop: boolean
}

/// Whether `value` names a group. Case-sensitive: it is matched against a URL
/// segment the codec itself wrote, never against a label.
export function isSettingsGroup(value: string): value is SettingsGroup {
    return (SETTINGS_GROUPS as readonly string[]).includes(value)
}

/// The groups this host offers, in order. The Desktop app group holds exactly
/// the settings only the desktop shell can honour, so the browser skin omits it
/// whole rather than offering an empty group (`web-ui`: *Desktop-Only Settings
/// Are Hidden in the Web UI*).
export function groupsForHost(host: SettingsHost): SettingsGroup[] {
    return SETTINGS_GROUPS.filter((group) => group !== "desktop" || host.desktop)
}

/// The group to show for an address naming `group`: the group itself when this
/// host offers it, otherwise the default. The codec decodes every group on
/// every host (it is host-independent by requirement), so this is where an
/// address the host cannot honour is caught.
export function effectiveSettingsGroup(group: SettingsGroup, host: SettingsHost): SettingsGroup {
    return groupsForHost(host).includes(group) ? group : DEFAULT_SETTINGS_GROUP
}
