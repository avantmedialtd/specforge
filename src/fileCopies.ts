import type { RegisteredWorkspace, WorkspaceFileRow } from "./types"

/// Per-file copy handling for the workspace file browser's pooled listing.
///
/// A Repo group's listing is the union of every tracked worktree's markdown,
/// de-duplicated on the root-relative path, so one row can be held by several
/// worktrees. Which worktree's bytes the preview renders is a per-preview
/// choice that never enters the Address (`view-routing`: *A file address
/// carries no worktree segment*), which is why these are plain functions over
/// the listing rather than routing helpers.

/// The last path segment of `path`, for a forward- or back-slashed path.
function basename(path: string): string {
    const parts = path.split(/[\\/]/).filter(Boolean)
    return parts.length > 0 ? parts[parts.length - 1]! : path
}

/// The row for `path`, or null when the pooled listing has no such file — the
/// case a file address into a path no tracked worktree holds resolves to.
export function rowForPath(
    rows: WorkspaceFileRow[],
    path: string | null,
): WorkspaceFileRow | null {
    if (!path) return null
    return rows.find((r) => r.path === path) ?? null
}

/// The worktree paths holding `row`, in the listing's order.
export function copyWorktrees(row: WorkspaceFileRow | null): string[] {
    return row?.copies.map((c) => c.worktreePath) ?? []
}

/// Which copy opens first: the repository's main worktree when it holds the
/// file, otherwise the first copy that does (`workspace-file-browser`: *The
/// main worktree's copy opens first*, *A file absent from the main worktree
/// still opens*; `view-routing`: *A repository file address resolves to a
/// default copy*).
///
/// `null` when the row has no copies at all, which is the listing saying the
/// path exists in no tracked worktree.
export function defaultCopy(
    worktrees: string[],
    mainWorktree: string | null,
): string | null {
    if (mainWorktree && worktrees.includes(mainWorktree)) return mainWorktree
    return worktrees[0] ?? null
}

/// The copy actually rendered: the pinned choice while it still holds the file,
/// otherwise the default.
///
/// The pin is what keeps a refresh from silently re-pointing the preview — the
/// copy order is derived from the tracked worktree set, so adding or removing a
/// worktree can change which copy is first while the reader is looking at
/// another one. A pinned copy that the refresh says no longer holds the file
/// (deleted in that worktree) falls back rather than reading a file that is
/// gone.
export function resolveCopy(
    worktrees: string[],
    pinned: string | null,
    mainWorktree: string | null,
): string | null {
    if (pinned && worktrees.includes(pinned)) return pinned
    return defaultCopy(worktrees, mainWorktree)
}

/// The paths whose copies differ on disk, for marking tree rows
/// (`workspace-file-browser`: *A file differing between worktrees is marked*).
///
/// A set, not a filtered list: the folder tree is derived from the path list
/// alone and consults this per row, so the hierarchy renders whether or not
/// divergence is known yet (*The tree renders before divergence is known*).
export function divergentPaths(rows: WorkspaceFileRow[]): Set<string> {
    return new Set(rows.filter((r) => r.differs).map((r) => r.path))
}

/// A copy's base label: the worktree's own display name when the override names
/// that folder alone, else the worktree folder's basename. **Never the branch**
/// — a worktree hosts whatever branch it happens to be on, and the file being
/// previewed has no relationship to it.
///
/// A repository group's display-name override is stored per repository, so
/// every worktree of it shares one; labelling copies with that would print the
/// same name against each and name nothing. It is therefore used only where the
/// presentation key is the folder's own path, which is a flat workspace.
function baseCopyLabel(
    worktree: string,
    workspaces: RegisteredWorkspace[],
): string {
    const ws = workspaces.find((w) => w.uri === worktree)
    if (ws && ws.repoId === null && ws.displayName) return ws.displayName
    return basename(worktree)
}

/// Labels for a row's copies, one per copy in `worktrees` order.
///
/// Two worktrees can share a basename in different parents, which would label
/// both identically and disambiguate nothing. Only the colliding labels are
/// qualified — with the worktree path, the one field that is unique by
/// construction — so the common case stays short.
export function copyLabels(
    worktrees: string[],
    workspaces: RegisteredWorkspace[],
): string[] {
    const bases = worktrees.map((w) => baseCopyLabel(w, workspaces))
    return bases.map((b, i) =>
        bases.indexOf(b) === bases.lastIndexOf(b) ? b : `${b} · ${worktrees[i]!}`,
    )
}
