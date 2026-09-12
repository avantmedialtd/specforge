// Address -> WorkspaceTree node path: a pure function built from the exact
// compositional id helpers `WorkspaceTree.tsx` itself renders (`repoId`,
// `flatWorkspaceId`, `logicalChangeId`, `changeRowId`) — imported, not
// reimplemented, so a reveal can never target an id the tree wouldn't
// recognise (design.md's *Tree reveal is the fiddliest part of the
// implementation* risk).
//
// Resolution (not re-derivation) does the address -> view lookup: this module
// calls `resolveAddress` and then classifies the resolved target's
// workspace/root against `views` the same way `renderTargetToAddress`
// already has to, via the same exported helpers.
//
// The path is at most TWO elements, because the tree is two levels: the
// container row (a repo group or a flat workspace) and the change row beneath
// it. Which ARTIFACT and which INSTANCE the address names is shown by the
// change header, not by the tree (`view-routing`: *Navigation Reveal Is
// Transient*), so no segment below the change row exists to return.
//
// The path is still built by CONSTRUCTION from each compositional call's own
// intermediate result, never by splitting the leaf id string on "/" (A2:
// container ids embed absolute filesystem paths, so a blind "/"-split ancestor
// can equal a DIFFERENT real node's id whenever one registered path is a
// directory prefix of another — this very repo's own `.claude/worktrees/<name>`
// layout is exactly that shape).

import {
    changeRowId,
    flatWorkspaceId,
    logicalChangeId,
    repoId,
} from "../components/WorkspaceTree"
import type { WorkspaceView } from "../types"
import type { Address } from "./address"
import { findViewByRoot, findWorkspaceMatch, resolveAddress } from "./resolve"

/// The WorkspaceTree node path `address` names, root-to-leaf inclusive, or
/// `null` when it resolves to nothing the tree renders (home / settings /
/// archive / a commit / an ambiguous, disabled or not-found address; a parked
/// row has no tree node to reveal, which is why no registered listing is
/// passed to `resolveAddress` here) — the caller
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

            // A repo group's change row is keyed by the LOGICAL change, not by
            // the instance the address names: every instance of one change
            // shares one row, and the addressed instance is marked in the
            // change header's switcher instead.
            if (found.view.kind === "repo" && found.logicalChangeName !== undefined) {
                return [
                    repoId(found.view.repoId),
                    logicalChangeId(found.view.repoId, found.logicalChangeName),
                ]
            }
            const wsId = flatWorkspaceId(target.workspace)
            return [wsId, changeRowId(wsId, target.changeId)]
        }
    }
}
