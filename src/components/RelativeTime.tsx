import { useTickingNow } from "../hooks/useTickingNow"
import { formatRelativeTime } from "../relativeTime"

/// A relative time that keeps itself current.
///
/// The one rendering of "how long ago" in the application. The sidebar's
/// instance rows, the Dashboard's ships feed and the detail pane's identity
/// header all reach it, so a single value cannot be spelled two ways on two
/// surfaces the user is looking at simultaneously.
///
/// Renders text only, with no element of its own, so each caller keeps its own
/// wrapper and styling — a dense sidebar row and a reserved-width header box
/// want different boxes around the same words.
export function RelativeTime({ unixSeconds }: { unixSeconds: number }) {
    const now = useTickingNow(unixSeconds)
    return <>{formatRelativeTime(unixSeconds, now)}</>
}
