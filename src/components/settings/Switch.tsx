interface SwitchProps {
    id?: string
    checked: boolean
    onChange: (checked: boolean) => void
    /// While the stored value is still loading, or a write is in flight.
    disabled?: boolean
    /// Only for a switch with no row title to name it — a registered-workspace
    /// row. Every settings row names its switch through its title's
    /// `<label htmlFor>` instead.
    ariaLabel?: string
    title?: string
}

/// The settings switch (`visual-identity`: *Settings Switch Control*).
///
/// A native checkbox exposed as a switch, so it keeps everything the native
/// control gives for free — Space flips it, a `<label htmlFor>` operates it,
/// its checked state reaches assistive technology — while the visible track
/// and knob are drawn by a sibling, not by the platform's checkbox appearance.
/// The input sits invisibly over the track and takes every click on it.
///
/// "On" is drawn in accent INK — the track's edge and the knob — over a
/// neutral track, never an accent fill, which *Accent Color* reserves for four
/// places. The knob's position carries the state, so it reads without colour.
export function Switch({ id, checked, onChange, disabled, ariaLabel, title }: SwitchProps) {
    return (
        <span className="settings-switch" title={title}>
            <input
                id={id}
                type="checkbox"
                role="switch"
                className="settings-switch-input"
                checked={checked}
                disabled={disabled}
                aria-label={ariaLabel}
                onChange={(e) => onChange(e.currentTarget.checked)}
            />
            <span className="settings-switch-track" aria-hidden="true">
                <span className="settings-switch-knob" />
            </span>
        </span>
    )
}
