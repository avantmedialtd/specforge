// Two marks the sidebar's change row and the detail pane's change header both
// render, kept in one module so the two surfaces cannot drift.
//
// They are visible at the same time and routinely describe the same change:
// the row's progress meter and the Tasks tab's are fed by the same counts
// (`spec-browser`: *Artifact Tab Strip in the Change Header* — "the same rule
// the change row applies, fed by the same counts, so the two never disagree on
// one screen"), and the divergence label moved from the instance row to the
// instance switcher while staying the same label (*Per-Instance Divergence
// Label*).

import type { DivergenceLabel } from "../types"

/// Task-progress meter — a fixed-width outlined track with an --ok fill whose
/// width is completed/total. Renders no inline digits; the exact count lives
/// in the `title` tooltip and the `progressbar` aria attributes. Renders
/// nothing when there are no parseable tasks. Callers hide it at 100% (the
/// trailing ✓ takes over), so the meter only ever depicts in-progress work
/// (`visual-identity`: *Task Progress Meter*).
export function TaskProgress({
    completed,
    total,
}: {
    completed: number
    total: number
}) {
    if (total <= 0) return null
    const fraction = Math.max(0, Math.min(1, completed / total))
    const label = `${completed} of ${total} tasks`
    return (
        <span
            className="task-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={total}
            aria-valuenow={completed}
            aria-label={label}
            title={label}
        >
            <span
                className="task-progress-fill"
                style={{ width: `${fraction * 100}%` }}
            />
        </span>
    )
}

/// The `[diverged]` / `[stale]` label for one instance — on the change row for
/// a singleton, in the change header's switcher for a change living in several
/// worktrees (`spec-browser`: *Per-Instance Divergence Label*).
export function DivergenceChip({ label }: { label: DivergenceLabel }) {
    const text = label === "diverged" ? "diverged" : "stale"
    const tone = label === "diverged" ? "chip--warn" : "chip--muted"
    return <span className={`chip ${tone}`}>{text}</span>
}
