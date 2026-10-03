import type { PanelPosition } from "../../types"

/// The four slots a pull-request panel can occupy, in the order the Layout
/// group offers them. Shared by the Layout group, where a slot is chosen, and
/// by an enabled integration's card, which names the slot its panel is in.
export const PANEL_POSITIONS: { value: PanelPosition; label: string }[] = [
    { value: "left-top", label: "Sidebar top" },
    { value: "left-bottom", label: "Sidebar bottom" },
    { value: "right-top", label: "Rail top" },
    { value: "right-bottom", label: "Rail bottom" },
]

export function panelPositionLabel(position: PanelPosition): string {
    return PANEL_POSITIONS.find((p) => p.value === position)?.label ?? position
}
