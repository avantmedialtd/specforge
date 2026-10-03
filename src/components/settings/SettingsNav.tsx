import { SETTINGS_GROUP_LABELS, type SettingsGroup } from "../../settingsGroups"

interface SettingsNavProps {
    /// The groups this host offers, in order.
    groups: SettingsGroup[]
    current: SettingsGroup
    onSelect: (group: SettingsGroup) => void
}

/// Navigation between the Settings view's groups (`settings-view`: *Group
/// Navigation Follows the View's Own Width*).
///
/// Both forms are always rendered and the stylesheet shows one: a list beside
/// the group while the view is at least 640px wide, and a native select above
/// it below that — the 320px center pane of a default-sized window included.
/// The select is the default and the list the `@container` enhancement, so an
/// engine without container queries still gets a working switcher. Whichever
/// form is hidden is `display: none`, which also takes it out of the
/// accessibility tree. Choosing a group leaves focus where it was.
export function SettingsNav({ groups, current, onSelect }: SettingsNavProps) {
    const choose = (group: SettingsGroup) => {
        if (group !== current) onSelect(group)
    }
    return (
        <nav className="settings-nav" aria-label="Settings groups">
            <ul className="settings-nav-list">
                {groups.map((group) => (
                    <li key={group}>
                        <button
                            type="button"
                            className="settings-nav-item"
                            aria-current={group === current ? "page" : undefined}
                            onClick={() => choose(group)}
                        >
                            {SETTINGS_GROUP_LABELS[group]}
                        </button>
                    </li>
                ))}
            </ul>
            <select
                className="settings-nav-select"
                aria-label="Settings group"
                value={current}
                onChange={(e) => choose(e.target.value as SettingsGroup)}
            >
                {groups.map((group) => (
                    <option key={group} value={group}>
                        {SETTINGS_GROUP_LABELS[group]}
                    </option>
                ))}
            </select>
        </nav>
    )
}
