// Pull-request ↔ worktree links, as the frontend reads them.
//
// The join itself is done in the headless app layer (`get_pull_request_links`,
// `openspec_app::pull_request_links`), where every tracked worktree's branch
// and every repository's remotes are known. What is left for the frontend —
// looking a worktree or a pull request up in the snapshot, choosing where a
// worktree marker lands, and wording the markers — lives here as pure
// functions, for the reason `changeNavigation.ts` gives: JSX is not exercised
// by `bun test` and a frontend-only diff short-circuits the mutation gate, so
// these tests are the only coverage this logic gets.

import { defaultArtifactFor, worktreeBasename } from "./changeNavigation"
import type {
    ArtifactRenderTarget,
    FilesRenderTarget,
    LinkedPullRequest,
    LinkedWorktree,
    PullRequestLinks,
    WorkspaceView,
} from "./types"

/// The snapshot before the first read, and whenever a read fails: nothing is
/// linked, so every surface renders exactly as it does without links.
export const EMPTY_LINKS: PullRequestLinks = { worktrees: [], pullRequests: [] }

/// The pull requests linked to the worktree at `worktreePath`, in the order
/// the snapshot gives them (BitBucket, then GitHub authored, then GitHub
/// review-requested). Empty when none are.
export function linksForWorktree(
    links: PullRequestLinks | null | undefined,
    worktreePath: string,
): LinkedPullRequest[] {
    if (!links) return []
    return links.worktrees.find((entry) => entry.worktreePath === worktreePath)?.pullRequests ?? []
}

/// The worktrees linked to the pull request at `url`, main worktrees first
/// and then by path, as the snapshot orders them. A row without a web URL links
/// nothing, so the empty URL never matches.
export function worktreesForPullRequest(
    links: PullRequestLinks | null | undefined,
    url: string,
): LinkedWorktree[] {
    if (!links || url === "") return []
    return links.pullRequests.find((entry) => entry.url === url)?.worktrees ?? []
}

/// Where a pull-request row's worktree marker lands (`pull-request-worktree-
/// links`: *Pull-Request Rows Lead to Their Worktree*, design D9):
///
/// - the default artifact of the one active change the worktree hosts, read
///   from that worktree;
/// - when it hosts several, the default artifact of the most recently
///   modified one (ties keep aggregation order);
/// - when it hosts none that has an artifact to open, the repository's file
///   browser — there is no worktree-level address to land on instead;
/// - `null` when the repository is not among the views (a disabled or removed
///   repository), which leaves the marker inert.
///
/// Returned as a render target so `App` mints the address through the one
/// `renderTargetToAddress` every other navigation uses (it adds the instance
/// token for a multi-worktree change, and scopes the file browser).
export function worktreeDestination(
    views: WorkspaceView[],
    repoId: string,
    worktreePath: string,
): ArtifactRenderTarget | FilesRenderTarget | null {
    const view = views.find((v) => v.kind === "repo" && v.repoId === repoId)
    if (!view || view.kind !== "repo") return null
    let best: { target: ArtifactRenderTarget; modifiedAt: number } | null = null
    for (const logical of view.active) {
        for (const instance of logical.instances) {
            if (instance.worktreePath !== worktreePath) continue
            const tab = defaultArtifactFor(instance.change.artifacts)
            if (!tab) continue
            if (best && instance.modifiedAt <= best.modifiedAt) continue
            best = {
                modifiedAt: instance.modifiedAt,
                target: {
                    kind: "artifact",
                    workspace: worktreePath,
                    changeId: logical.name,
                    artifactKind: tab.kind,
                    ...(tab.capability !== undefined ? { capability: tab.capability } : {}),
                },
            }
        }
    }
    return best ? best.target : { kind: "files", root: view.repoId }
}

/// How a linked worktree names itself: its branch, or its folder basename when
/// the branch is not known.
export function worktreeName(worktree: LinkedWorktree): string {
    return worktree.branch ?? worktreeBasename(worktree.worktreePath)
}

/// The worktree marker beside a linked pull-request row: its visible text
/// names the FIRST linked worktree (the one it navigates to), and its label —
/// used for both the tooltip and the accessible name — lists every linked
/// worktree. `null` when the pull request is linked to none, so the row renders
/// exactly as it does without links.
export function worktreeMarker(
    worktrees: LinkedWorktree[],
): { text: string; label: string } | null {
    const first = worktrees[0]
    if (!first) return null
    const listed = worktrees
        .map((w) => `${worktreeName(w)} (${w.worktreePath})`)
        .join(", ")
    return {
        text: worktreeName(first),
        label: `Open in SpecForge: ${listed}`,
    }
}

/// The instance switcher's passive marker for an instance's linked pull
/// requests: `#<first>` and `+N` for the rest, or `null` when none are linked
/// (`spec-browser`: *Pull-Request Marker in the Instance Switcher*).
export function switcherMarkerText(numbers: number[]): string | null {
    const [first, ...rest] = numbers
    if (first === undefined) return null
    return rest.length > 0 ? `#${first} +${rest.length}` : `#${first}`
}

/// How many pull-request chips the change header shows before summarising the
/// rest in one passive `+N` chip (`spec-browser`: *Pull-Request Chip in the
/// Change Header*).
export const MAX_HEADER_CHIPS = 2

/// The header's chips: the first `MAX_HEADER_CHIPS` shown as controls, the
/// rest folded into the `+N` summary.
export function headerChips(pullRequests: LinkedPullRequest[]): {
    shown: LinkedPullRequest[]
    overflow: LinkedPullRequest[]
} {
    return {
        shown: pullRequests.slice(0, MAX_HEADER_CHIPS),
        overflow: pullRequests.slice(MAX_HEADER_CHIPS),
    }
}
