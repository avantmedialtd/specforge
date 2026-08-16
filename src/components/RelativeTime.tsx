import { useRelativeTime } from "../hooks/useRelativeTime"

/// A relative time that keeps itself current.
///
/// The one rendering of "how long ago" in the application. The sidebar's
/// instance rows and the Dashboard's ships feed use it directly; the detail
/// pane's identity header shares its `useRelativeTime` hook instead, because it
/// needs the same text a second time for a tooltip and must not have two
/// spellings of one instant.
///
/// Renders text only, with no element of its own, so each caller keeps its own
/// wrapper and styling — a dense sidebar row and a reserved-width header box
/// want different boxes around the same words.
export function RelativeTime({ unixSeconds }: { unixSeconds: number }) {
    return <>{useRelativeTime(unixSeconds)}</>
}
