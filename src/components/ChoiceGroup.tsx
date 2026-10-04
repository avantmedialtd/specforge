import type { KeyboardEvent } from "react"

export interface ChoiceOption<T extends string> {
    value: T
    /// The option's visible text, which is also its accessible name.
    label: string
}

interface ChoiceGroupProps<T extends string> {
    options: readonly ChoiceOption<T>[]
    value: T
    onChange: (value: T) => void
    /// The group's accessible name.
    label: string
    /// The id of visible text that describes the group's state, such as the
    /// diff view's "Too narrow — showing unified".
    describedBy?: string
}

/// The option an arrow key moves to and selects, from the checked option at
/// `current`, or null for any other key. ArrowRight and ArrowDown take the
/// next option and ArrowLeft and ArrowUp the previous one, wrapping at either
/// end. With nothing checked, the arrows start from the matching end.
export function choiceKeyTarget(key: string, current: number, count: number): number | null {
    const step =
        key === "ArrowRight" || key === "ArrowDown"
            ? 1
            : key === "ArrowLeft" || key === "ArrowUp"
              ? -1
              : 0
    if (step === 0 || count <= 0) return null
    if (current < 0 || current >= count) return step === 1 ? 0 : count - 1
    return (current + step + count) % count
}

/// The option that is the group's single Tab stop: the checked one, or the
/// first while none is checked, so the group can always be reached.
export function choiceTabStop(checked: number, count: number): number {
    return checked >= 0 && checked < count ? checked : 0
}

/// A radio group of text-labelled choices, styled as Settings' choice rows
/// (`.settings-choice-row`, `.settings-choice`), with the keyboard contract of
/// the workspace tint palette (`handlePaletteKeyDown` in
/// `settings/WorkspacesGroup.tsx`): the checked option is the group's single
/// Tab stop, and the arrow keys move focus to the next or previous option and
/// select it, wrapping at either end. The diff view's layout control is one
/// (`diff-view`: *Keyboard and Accessibility*). Settings' own choice rows keep
/// their markup and keyboard behaviour.
///
/// Choosing the checked option again changes nothing, so a host's `onChange`
/// hears only real changes.
export function ChoiceGroup<T extends string>({
    options,
    value,
    onChange,
    label,
    describedBy,
}: ChoiceGroupProps<T>) {
    const checked = options.findIndex((option) => option.value === value)
    const tabStop = choiceTabStop(checked, options.length)

    const choose = (next: T) => {
        if (next !== value) onChange(next)
    }

    const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
        const target = choiceKeyTarget(e.key, checked, options.length)
        if (target === null) return
        e.preventDefault()
        choose(options[target].value)
        e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[target]?.focus()
    }

    return (
        <div
            className="settings-choice-row"
            role="radiogroup"
            aria-label={label}
            aria-describedby={describedBy}
            onKeyDown={handleKeyDown}
        >
            {options.map((option, index) => (
                <button
                    key={option.value}
                    type="button"
                    role="radio"
                    aria-checked={index === checked}
                    tabIndex={index === tabStop ? 0 : -1}
                    className="settings-choice"
                    onClick={() => choose(option.value)}
                >
                    {option.label}
                </button>
            ))}
        </div>
    )
}
