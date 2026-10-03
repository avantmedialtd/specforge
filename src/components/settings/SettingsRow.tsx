import type { ReactNode } from "react"

interface SettingsRowProps {
    /// The setting's name. Rendered as the `<label>` of `controlId` when one is
    /// given, so it is the control's accessible name and activating it
    /// operates the control — a switch flips, a field takes focus.
    title: ReactNode
    /// The id of the input the title labels. Omit it for a control that is
    /// named another way — a radio group names itself from `titleId`.
    controlId?: string
    /// An id for the title, for a control that refers to it with
    /// `aria-labelledby` rather than being labelled by it.
    titleId?: string
    description?: ReactNode
    control: ReactNode
    /// Why this row's last write did not take, shown on the row itself
    /// (`settings-view`: *Settings Persist by One Rule*).
    error?: string | null
    /// `inline` puts a compact control (a switch) beside the text, dropping it
    /// below only when the row is too narrow for both. `stacked` always puts
    /// the control below the text — for a wide one, such as a choice row or a
    /// text field, which would otherwise crowd the title.
    layout?: "inline" | "stacked"
}

/// One setting (`settings-view`: *Settings Rows*): its title and description
/// at the leading edge, its control at the trailing edge.
export function SettingsRow({
    title,
    controlId,
    titleId,
    description,
    control,
    error,
    layout = "inline",
}: SettingsRowProps) {
    return (
        <div className={`settings-row settings-row--${layout}`}>
            <div className="settings-row-text">
                {controlId ? (
                    <label className="settings-row-title" htmlFor={controlId} id={titleId}>
                        {title}
                    </label>
                ) : (
                    <span className="settings-row-title" id={titleId}>
                        {title}
                    </span>
                )}
                {description && <div className="settings-row-description">{description}</div>}
            </div>
            <div className="settings-row-control">{control}</div>
            {error && (
                <p className="settings-error settings-row-error" role="alert">
                    {error}
                </p>
            )}
        </div>
    )
}
