// Address -> WorkspaceTree node path: a pure function built from the exact
// compositional id helpers `WorkspaceTree.tsx` itself renders and persists
// collapse state under (`repoId`, `logicalChangeId`, `instanceId`,
// `artifactNodeId`, etc.) — imported, not reimplemented, so a reveal can
// never target an id the tree wouldn't recognise (design.md's *Tree reveal
// is the fiddliest part of the implementation* risk).
//
// Resolution (not re-derivation) does the address -> view lookup: this module
// calls `resolveAddress` and then classifies the resolved target's
// workspace/root against `views` the same way `renderTargetToAddress`
// already has to, via the same exported helpers.
//
// The full root-to-leaf PATH is returned — not just the leaf id — built by
// construction from each compositional call's own intermediate result,
// never by splitting the leaf id string on "/" (A2: node ids embed absolute
// filesystem paths, e.g. `instanceId`'s worktree segment, so a blind
// "/"-split ancestor can equal a DIFFERENT real node's id whenever one
// registered path is a directory prefix of another — this very repo's own
// `.claude/worktrees/<name>` layout is exactly that shape: a worktree
// instance's id has the main-worktree instance's id as a literal substring).

import {
    artifactNodeId,
    changeRowId,
    flatWorkspaceId,
    instanceId,
    logicalChangeId,
    repoId,
    specNodeId,
} from "../components/WorkspaceTree"
import type { WorkspaceView } from "../types"
import type { Address } from "./address"
import { findViewByRoot, findWorkspaceMatch, resolveAddress } from "./resolve"

/// The WorkspaceTree node path `address` names, root-to-leaf inclusive, or
/// `null` when it resolves to nothing the tree renders (home / settings /
/// archive / a commit / an ambiguous or not-found address) — the caller
/// (`App.tsx`) treats `null` as "clear any transient reveal," and the last
/// element as the node to select/highlight.
export function addressToNodePath(address: Address, views: WorkspaceView[]): string[] | null {
    const result = resolveAddress(address, views)
    if (result.status !== "resolved" || result.view.kind !== "target") return null
    const target = result.view.target

    switch (target.kind) {
        case "dashboard":
        case "commit":
            return null
        case "files": {
            const view = findViewByRoot(target.root, views)
            if (!view) return null
            return [view.kind === "repo" ? repoId(view.repoId) : flatWorkspaceId(view.workspace.uri)]
        }
        case "artifact": {
            const found = findWorkspaceMatch(target.workspace, views, target.changeId)
            if (!found) return null

            let path: string[]
            let containerId: string
            if (found.view.kind === "repo" && found.logicalChangeName !== undefined) {
                containerId = instanceId(found.view.repoId, found.logicalChangeName, target.workspace)
                path = [
                    repoId(found.view.repoId),
                    // Not always a real row on its own — only rendered when
                    // the change has more than one instance (see
                    // `LogicalChangeRow`) — but `forcedOpen` membership is
                    // simply never queried for an id with no matching row,
                    // so including it unconditionally is harmless.
                    logicalChangeId(found.view.repoId, found.logicalChangeName),
                    containerId,
                ]
            } else {
                const wsId = flatWorkspaceId(target.workspace)
                containerId = changeRowId(wsId, target.changeId)
                path = [wsId, containerId]
            }

            if (target.artifactKind === "spec") {
                path.push(artifactNodeId(containerId, target.changeId, "specs"))
                path.push(specNodeId(containerId, target.changeId, target.capability ?? ""))
            } else {
                path.push(artifactNodeId(containerId, target.changeId, target.artifactKind))
            }
            return path
        }
    }
}
