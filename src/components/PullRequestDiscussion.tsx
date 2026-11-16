import type { ReactNode } from "react"
import { minimisedText, threadPlace } from "../pullRequestView"
import type { ReviewThread } from "../types"
import { EscapedText } from "./DiffView"

export type ThreadFilter = "all" | "unresolved" | "outdated"

export function PullRequestDiscussion({
    threads, filter, onFilter, onJump, children,
}: {
    threads: readonly ReviewThread[]
    filter: ThreadFilter
    onFilter: (filter: ThreadFilter) => void
    onJump: (thread: ReviewThread) => void
    children: ReactNode
}) {
    const shown = threads.filter((thread) => filter === "all" ||
        (filter === "unresolved" && !thread.resolved) || (filter === "outdated" && thread.outdated))
    return <>
        {children}
        <section className="pull-request-view-section" id="pr-thread-index" tabIndex={-1}>
            <div className="pr-section-toolbar">
                <h2 className="pull-request-view-heading">Review threads</h2>
                <label>Show threads{" "}
                    <select value={filter} onChange={(event) => onFilter(event.currentTarget.value as ThreadFilter)}>
                        <option value="all">All</option>
                        <option value="unresolved">Unresolved</option>
                        <option value="outdated">Outdated</option>
                    </select>
                </label>
            </div>
            {shown.length === 0 && <p className="pull-request-view-empty">No matching threads.</p>}
            <ol className="pr-thread-index">
                {shown.map((thread) => {
                    const comment = thread.comments[0]
                    const excerpt = !comment || comment.deleted ? "This comment was deleted." :
                        comment.minimizedReason !== null ? minimisedText(comment.minimizedReason) :
                            comment.body.slice(0, 180)
                    return <li key={thread.id}>
                        <button type="button" className="pr-thread-jump" onClick={() => onJump(thread)}>
                            <span className="pr-thread-path"><EscapedText text={thread.path} /> · {threadPlace(thread)}</span>
                            <span>{comment?.author ?? "Deleted account"} · {thread.resolved ? "Resolved" : "Unresolved"}
                                {thread.outdated ? " · Outdated" : ""} · {Math.max(0, thread.comments.length - 1)} replies</span>
                            <span className="pr-thread-excerpt"><EscapedText text={excerpt} /></span>
                            <span className="pr-thread-jump-label">Jump to thread →</span>
                        </button>
                    </li>
                })}
            </ol>
        </section>
    </>
}
