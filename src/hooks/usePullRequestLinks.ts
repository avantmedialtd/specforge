import { useEffect, useState } from "react"
import {
    getPullRequestLinks,
    onBitbucketPullRequestsUpdated,
    onGithubPullRequestsUpdated,
} from "../api"
import { EMPTY_LINKS } from "../pullRequestLinks"
import type { PullRequestLinks, WorkspaceView } from "../types"

/// The pull-request ↔ worktree links snapshot (`pull-request-worktree-links`:
/// *The Pull-Request Links Snapshot*).
///
/// There is no event of its own: the links are derived from inputs that
/// already announce themselves, so the snapshot is re-read whenever the
/// workspace views change — a branch switch, a remote change and a worktree
/// appearing all arrive that way — and whenever either provider's pull-request
/// snapshot is announced as updated. A failed read leaves the last snapshot in
/// place; before the first read nothing is linked.
export function usePullRequestLinks(views: WorkspaceView[]): PullRequestLinks {
    const [links, setLinks] = useState<PullRequestLinks>(EMPTY_LINKS)
    // Bumped by either provider's update event; a dependency of the read
    // below, so both triggers funnel through one effect.
    const [pullRequestsVersion, setPullRequestsVersion] = useState(0)

    useEffect(() => {
        const bump = () => setPullRequestsVersion((v) => v + 1)
        const unlisteners = [onBitbucketPullRequestsUpdated(bump), onGithubPullRequestsUpdated(bump)]
        return () => {
            for (const unlisten of unlisteners) void unlisten.then((u) => u())
        }
    }, [])

    useEffect(() => {
        // A later read supersedes an earlier one still in flight, so an
        // out-of-order reply can never overwrite a newer snapshot.
        let current = true
        getPullRequestLinks()
            .then((next) => {
                if (current) setLinks(next)
            })
            .catch(() => {})
        return () => {
            current = false
        }
    }, [views, pullRequestsVersion])

    return links
}
