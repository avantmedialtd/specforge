// Naming an archived change's copies.
//
// Moved out of `ArchiveView.tsx` unchanged when the reader adopted the shared
// change header: the labels the old `<select>` offered are exactly the labels
// the switcher must offer, so pinning them by test is only possible if the
// function lives where `bun test` can reach it (`archive-browser`:
// *Read-Only Artifact Navigation* — "adopting the shared switcher changes what
// the control looks like, never what it says").

import type { SwitcherOption } from "./changeNavigation"
import type { ArchivedChangeCopy, RegisteredWorkspace } from "./types"

function basename(path: string): string {
    const parts = path.split(/[\\/]/).filter(Boolean)
    return parts.length > 0 ? parts[parts.length - 1]! : path
}

/// Identity of one copy within its row. The worktree path alone is not enough:
/// a single worktree can hold a dated archive directory and its legacy un-dated
/// twin, which are two copies of one logical change.
export function copyKey(copy: ArchivedChangeCopy): string {
    return `${copy.worktreePath}\u0000${copy.archiveDir}`
}

/// A copy's base label: the worktree's own display name when the override
/// names that folder alone, else the worktree folder's basename. **Never the
/// branch** (`archive-browser`: *Copies are named by workspace, never by
/// branch*) — the worktree an archived change is read from routinely hosts
/// other, active changes whose branch was never this change's.
///
/// A repository group's display-name override is stored per repository, so
/// every worktree of it shares one. Labelling copies with that would print the
/// same name against each and name nothing, so it is used only for a flat
/// workspace, whose presentation key is its own path.
function baseCopyLabel(
    copy: ArchivedChangeCopy,
    workspaces: RegisteredWorkspace[],
): string {
    const ws = workspaces.find((w) => w.uri === copy.worktreePath)
    if (ws && ws.repoId === null && ws.displayName) return ws.displayName
    return basename(copy.worktreePath)
}

/// Labels for a row's copies, one per copy in `copies` order.
///
/// Copies are never presented as interchangeable: archived content is read from
/// the working tree rather than from git, so two copies can genuinely differ.
/// Whenever their on-disk directories differ — different archive dates, or a
/// legacy un-dated twin — each label carries its own directory name, which is
/// also what tells apart two copies that would otherwise read identically.
export function copyLabels(
    copies: ArchivedChangeCopy[],
    workspaces: RegisteredWorkspace[],
): string[] {
    const bases = copies.map((c) => baseCopyLabel(c, workspaces))
    const dirsDiffer = new Set(copies.map((c) => c.archiveDir)).size > 1
    if (dirsDiffer) {
        return bases.map((b, i) => `${b} · ${copies[i]!.archiveDir}`)
    }
    // Two copies can share a base label without differing directories — two
    // worktrees whose folders have the same basename, in different parents.
    // Appending the directory there appends the SAME string to both and
    // disambiguates nothing, so fall back to the one field that is unique by
    // construction: the worktree path.
    const collides = bases.some((b, i) => bases.indexOf(b) !== i)
    if (!collides) return bases
    return bases.map((b, i) =>
        bases.indexOf(b) === bases.lastIndexOf(b)
            ? b
            : `${b} · ${copies[i]!.worktreePath}`,
    )
}

/// The change header's switcher controls for an archived change's copies.
///
/// Built here rather than in the component so the prohibition is structural:
/// every option leaves `branch`, `color` and `divergence` null, so no branch
/// can reach the reader through the switcher whatever the host worktree is on.
export function copyOptions(
    copies: ArchivedChangeCopy[],
    workspaces: RegisteredWorkspace[],
): SwitcherOption[] {
    const labels = copyLabels(copies, workspaces)
    return copies.map((copy, index) => ({
        key: copyKey(copy),
        label: labels[index]!,
        branch: null,
        color: null,
        divergence: null,
    }))
}
