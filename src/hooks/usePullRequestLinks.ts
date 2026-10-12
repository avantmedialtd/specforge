import { useCallback, useEffect, useState } from "react"
import {
    getPullRequestLinks,
    onBitbucketPullRequestsUpdated,
    onGithubPullRequestsUpdated,
} from "../api"
import { EMPTY_LINKS } from "../pullRequestLinks"
import { createRequestSequence, readLatest, type Subscribe } from "../refreshSignals"
import { shareStructure } from "../structuralShare"
import type { PullRequestLinks } from "../types"
import { useCoalescedRefetch } from "./useCoalescedRefetch"

/// The pull-request ↔ worktree links snapshot (`pull-request-worktree-links`:
/// *The Pull-Request Links Snapshot*, *An Unchanged Links Snapshot Is Not
/// Re-applied*).
///
/// There is no event of its own: the links are derived from inputs that
/// already announce themselves, so the snapshot is re-read after every
/// workspace views read — a branch switch, a remote change and a worktree
/// appearing all arrive that way — and whenever either provider's pull-request
/// snapshot is announced as updated. `onViewsRead` is `useWorkspaces`' signal,
/// not the views themselves: structural sharing keeps an equal views read's
/// identity, and a remote change can alter the links while the views stay
/// equal, so keying on `views` would miss exactly that read.
///
/// Reads go through one coalesced scheduler (a burst costs one read, and at
/// most one more after a read in flight), the newest reply wins, and a reply
/// equal to the held snapshot keeps it, so nothing derived from the links
/// renders again. A failed read leaves the last snapshot in place; before the
/// first read nothing is linked.
export function usePullRequestLinks(onViewsRead: Subscribe): PullRequestLinks {
    const [links, setLinks] = useState<PullRequestLinks>(EMPTY_LINKS)
    const [sequence] = useState(createRequestSequence)

    const read = useCallback(
        () =>
            readLatest(sequence, getPullRequestLinks, (next) =>
                setLinks((prev) => shareStructure(prev, next)),
            ),
        [sequence],
    )
    const schedule = useCoalescedRefetch(read)

    useEffect(() => {
        // The first read, as the hook mounts; every later one follows a views
        // read or an announcement.
        schedule()
        const unsubscribe = onViewsRead(schedule)
        const unlisteners = [
            onBitbucketPullRequestsUpdated(schedule),
            onGithubPullRequestsUpdated(schedule),
        ]
        return () => {
            unsubscribe()
            // Nothing still in flight lands after the hook lets go.
            sequence.cancel()
            for (const unlisten of unlisteners) void unlisten.then((u) => u())
        }
    }, [onViewsRead, schedule, sequence])

    return links
}
