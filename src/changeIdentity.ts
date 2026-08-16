// Identity of the change a reading surface is currently showing.
//
// The detail pane names the change whose artifact it renders (`spec-browser`:
// *Change Identity Header in the Detail Pane*), and the Archive reader names
// the archived change's on-disk directory (`archive-browser`: *Read-Only
// Artifact Navigation*). Both need a value derived from a render target rather
// than carried on one, so the derivations live here as pure functions — JSX
// cannot be exercised by `bun test`, and a frontend-only diff short-circuits
// the mutation gate, so these tests are the only coverage this logic gets.

import type { PaletteColor, WorkspaceView } from "./types"

/// The `archive/` path prefix `ArchiveView` prepends when it builds a render
/// target for an archived change, so `resolve_artifact_path` reads from
/// `openspec/changes/archive/<dir>` rather than `openspec/changes/<dir>`.
const ARCHIVE_PREFIX = "archive/"

/// Everything the branch chip needs: what it says, and what colour it says it
/// in.
export interface BranchChip {
    /// The worktree's branch, or null when there is none to name — in which
    /// case no chip is rendered at all.
    branch: string | null
    /// The owning workspace's configured palette colour, or null when it has
    /// none. Null means the chip renders in its neutral ink, never in a
    /// derived or arbitrary colour (`spec-browser`: *Change Identity Header in
    /// the Detail Pane*).
    color: PaletteColor | null
}

/// The branch chip for the worktree at `worktreePath`.
///
/// Both facts come from ONE walk and ONE match, rather than from a second
/// lookup for the colour. The colour must be the one belonging to the
/// workspace that owns the worktree the artifact was actually read from; two
/// independent traversals could each match a different instance and disagree
/// about which workspace that was, and nothing in their signatures would say
/// so. Resolving them together makes that disagreement unrepresentable.
///
/// Deliberately derived from the views rather than carried on
/// `ArtifactRenderTarget`: a target is built both by
/// `renderTargetForSelection` — which has the `ChangeInstance` in hand — and by
/// the routing layer resolving a URL address, which does not. A field populated
/// on only the first path would show the branch when the user clicks a row and
/// drop it when they open the same artifact from a link, which reads as a bug
/// rather than as an absent branch.
///
/// A null `branch` covers three distinct cases that all mean "no branch to
/// show": a flat workspace (whose `workspace.uri` matches no instance), a
/// detached-HEAD or bare worktree (whose instance carries `branch: null`), and
/// a path that is simply not among the tracked instances. The header renders no
/// chip for all three, which is correct for each.
///
/// The colour is reported from whatever instance matched, even when that
/// instance names no branch. It is the honest answer to "which workspace owns
/// this path", and the decision to render nothing belongs to the caller — not
/// to a lookup that would have to withhold a fact it holds.
///
/// A worktree hosting several active changes yields several instances at the
/// same path; they necessarily share a branch and a workspace, so the first
/// match is the answer and the scan can stop there.
export function branchChipForWorktree(
    worktreePath: string,
    views: WorkspaceView[],
): BranchChip {
    for (const view of views) {
        if (view.kind !== "repo") continue
        for (const logical of view.active) {
            for (const instance of logical.instances) {
                if (instance.worktreePath === worktreePath) {
                    return { branch: instance.branch, color: view.color }
                }
            }
        }
    }
    return { branch: null, color: null }
}

/// The full class list for a verbatim-identifier chip: the shared appearance
/// class, its palette tint when one applies, and the calling surface's own
/// layout class.
///
/// Emitted from here rather than written out at each call site, because the
/// defect this vocabulary exists to prevent was a hand-copy that fell out of
/// step. Unifying the stylesheet while leaving two components to hand-write
/// `ident-chip ident-chip--<colour> <layout>` would leave the same drift one
/// rename or one typo away, and it would be invisible: JSX is not exercised by
/// `bun test`, and a frontend-only diff short-circuits the mutation gate, so
/// nothing would fail. Living here — in the module both call sites already
/// import, and which `bun test` does cover — the class list has one definition
/// and one set of tests, exactly as the CSS now has one definition.
///
/// `layoutClass` is deliberately the caller's business: the tree ellipsizes
/// and shrinks, the identity header holds its size and wraps. Only appearance
/// is shared, and only appearance was ever duplicated.
export function identChipClass(
    color: PaletteColor | null,
    layoutClass: string,
): string {
    const tint = color ? ` ident-chip--${color}` : ""
    return `ident-chip${tint} ${layoutClass}`
}

/// Whether a render target's `changeId` addresses an ARCHIVED change.
///
/// The distinction matters to the identity header because an archived change
/// has no live worktree and therefore no branch to name — but its target's
/// `workspace` is still the registered worktree path the archive was read
/// from, which routinely DOES match a live instance hosting other, active
/// changes. Resolving a branch from that path would label an archived change
/// with whatever branch its host worktree happens to be on, which is not the
/// archived change's branch and was never true of it.
///
/// Derived from the id rather than passed in by the caller: the Archive reader
/// renders through the same `DetailPane`, so a flag a caller had to remember to
/// set would be one refactor away from being silently dropped.
export function isArchivedChangeId(changeId: string): boolean {
    return changeId.startsWith(ARCHIVE_PREFIX)
}

/// A render target's `changeId` as a bare directory name.
///
/// `ArchiveView` addresses an archived change as `archive/<YYYY-MM-DD>-<id>`,
/// because that prefix is what steers the artifact read into the archive
/// subtree. The prefix is a detail of the read path, not part of the folder's
/// name, so a header that displays the change's on-disk identity must drop it
/// — the value is meant to be copied and used as a filesystem identifier
/// (`archive-browser`: *Read-Only Artifact Navigation*).
///
/// An active change's id has no prefix and is returned unchanged.
export function changeDirectoryName(changeId: string): string {
    return isArchivedChangeId(changeId)
        ? changeId.slice(ARCHIVE_PREFIX.length)
        : changeId
}
