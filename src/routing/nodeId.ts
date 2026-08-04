// Address -> WorkspaceTree node id: a pure function built from the exact
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

import {
    artifactNodeId,
    changeRowId,
    flatWorkspaceId,
    instanceId,
    repoId,
    specNodeId,
} from "../components/WorkspaceTree"
import type { WorkspaceView } from "../types"
import type { Address } from "./address"
import { findViewByRoot, findWorkspaceMatch, resolveAddress } from "./resolve"

/// The WorkspaceTree node id `address` names, or `null` when it resolves to
/// nothing the tree renders (home / settings / archive / a commit / an
/// ambiguous or not-found address) — the caller (`App.tsx`) treats `null` as
/// "clear any transient reveal."
export function addressToNodeId(address: Address, views: WorkspaceView[]): string | null {
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
            return view.kind === "repo" ? repoId(view.repoId) : flatWorkspaceId(view.workspace.uri)
        }
        case "artifact": {
            const found = findWorkspaceMatch(target.workspace, views)
            if (!found) return null
            const containerId =
                found.view.kind === "repo" && found.logicalChangeName !== undefined
                    ? instanceId(found.view.repoId, found.logicalChangeName, target.workspace)
                    : changeRowId(flatWorkspaceId(target.workspace), target.changeId)
            return target.artifactKind === "spec"
                ? specNodeId(containerId, target.changeId, target.capability ?? "")
                : artifactNodeId(containerId, target.changeId, target.artifactKind)
        }
    }
}
