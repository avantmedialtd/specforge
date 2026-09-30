import { isWeb, openPullRequest } from "../api"
import { identChipClass } from "../changeIdentity"
import type { LinkedPullRequest, PullRequestProvider } from "../types"
import { checksLabel, reviewCellTitle } from "./PullRequestPanel"

// The change header's pull-request chips (`spec-browser`: *Pull-Request Chip in
// the Change Header*). A chip names a pull request linked to the worktree the
// artifact is read from, carries the same draft / checks / conflict treatments
// the panels use, and opens the pull request exactly as a panel row does.

const PROVIDER_NAMES: Record<PullRequestProvider, string> = {
    bitbucket: "BitBucket",
    github: "GitHub",
}

/// A chip's tooltip and accessible name: the provider, the destination
/// repository, the number and title, whether it is the viewer's own or awaiting
/// their review, the review summary in words, and the checks state in words —
/// plus the draft and conflict markers the chip shows, so nothing visible is
/// missing from what assistive technology reads.
export function pullRequestChipLabel(pr: LinkedPullRequest): string {
    const parts = [
        `${PROVIDER_NAMES[pr.provider]} pull request #${pr.id} in ${pr.repoFullName || "an unknown repository"}: ${pr.title}`,
        pr.role === "authored" ? "Yours" : "Awaiting your review",
        reviewCellTitle(pr.review),
    ]
    if (pr.checks) parts.push(checksLabel(pr.checks))
    if (pr.draft) parts.push("Draft")
    if (pr.conflicting) parts.push("Merge conflicts")
    return parts.join(". ")
}

/// The `+N` summary's tooltip: the pull requests it stands for, one per line.
export function overflowChipLabel(pullRequests: LinkedPullRequest[]): string {
    return [
        `${pullRequests.length} more linked pull request${pullRequests.length === 1 ? "" : "s"}:`,
        ...pullRequests.map((pr) => `${PROVIDER_NAMES[pr.provider]} #${pr.id} — ${pr.title}`),
    ].join("\n")
}

/// One linked pull request as a header chip. A SIBLING of the change name,
/// never a child — the name carries `user-select: all`, so a nested chip would
/// be swept into its copy. Neutral ink, never the workspace tint, so it is not
/// mistaken for the branch chip. On the desktop a button through the
/// snapshot-scoped `open_pull_request`; in the browser skin a new-tab link that
/// never navigates the serving page.
export function PullRequestChip({ pr }: { pr: LinkedPullRequest }) {
    const label = pullRequestChipLabel(pr)
    const className = identChipClass(null, "pull-request-chip")
    const content = (
        <>
            <span className="pull-request-chip-number">#{pr.id}</span>
            {pr.draft && <span className="pull-request-draft">Draft</span>}
            {pr.checks && (
                <span
                    className={`pull-request-checks pull-request-checks--${pr.checks}`}
                    aria-hidden="true"
                />
            )}
            {pr.conflicting && <span className="pull-request-conflict">Conflicts</span>}
        </>
    )
    if (isWeb()) {
        return (
            <a
                className={className}
                href={pr.url}
                target="_blank"
                rel="noopener noreferrer"
                title={label}
                aria-label={label}
            >
                {content}
            </a>
        )
    }
    return (
        <button
            type="button"
            className={className}
            // A refusal (the row left the snapshot since the links were read)
            // or an opener failure is quiet: the chip neither navigates nor
            // throws.
            onClick={() => void openPullRequest(pr.url).catch(() => {})}
            title={label}
            aria-label={label}
        >
            {content}
        </button>
    )
}

/// The passive `+N` chip standing for the linked pull requests beyond the
/// first two. Informational only — not a control, not a tab stop.
export function PullRequestOverflowChip({ pullRequests }: { pullRequests: LinkedPullRequest[] }) {
    if (pullRequests.length === 0) return null
    const label = overflowChipLabel(pullRequests)
    return (
        <span
            className={identChipClass(null, "pull-request-chip pull-request-chip--overflow")}
            title={label}
            aria-label={label}
        >
            +{pullRequests.length}
        </span>
    )
}
