// Top-level-row identity over the *registered* workspace listing.
//
// `list_workspaces` is the frontend's only sight of a parked row: the
// aggregated view (`get_workspace_views`) drops every disabled top-level row
// before it reaches any frontend (`workspace-registry`: *Disabled Rows
// Excluded From the Tree Pane*, design.md D3), while the listing keeps it and
// flags it. Everything here therefore works from `RegisteredWorkspace[]` — the
// one shape that still carries a parked row's name, identity and flag — and
// never from path arithmetic over `repoId`, whose relationship to the row's
// name is a git-layout detail (a submodule's git dir lives under
// `<super>/.git/modules/<sub>`, a `--separate-git-dir` checkout's anywhere at
// all).
//
// The listing is per *registered folder*, not per row: two registered
// worktrees of one repository are two entries sharing a single
// `PresentationKey::Repo` — one disabled flag, one display name, one tint
// (`workspace-registry`: *Workspace Disable State*). `rowKey` is that shared
// identity, and every count/lookup below is expressed in terms of it.

import { shortHash, slugify } from "./routing/slug"
import type { RegisteredWorkspace, ShipEntry, WorkspaceView } from "./types"

/// Mirrors `PresentationKey` (crates/openspec-core/src/presentation.rs): a
/// repository group is keyed by its git common dir, a flat workspace by its
/// own path. Two registered worktrees of one repository share ONE key — and so
/// one disabled flag. The `repo:`/`flat:` prefix keeps the key total the way
/// the Rust enum is, so a flat workspace registered at a path that happens to
/// equal another row's `repoId` string can never collide with it.
export function rowKey(ws: RegisteredWorkspace): string {
    return ws.repoId !== null ? `repo:${ws.repoId}` : `flat:${ws.uri}`
}

/// One entry per DISABLED top-level row — i.e. per row the tree actually
/// drops, which is what any "how much is hidden" figure has to count.
/// Deliberately not `all.filter((w) => w.disabled)`: that counts registered
/// folders, so parking one repository that the user registered at two
/// worktrees would report two rows while the tree loses exactly one.
export function disabledRows(all: RegisteredWorkspace[]): RegisteredWorkspace[] {
    const seen = new Map<string, RegisteredWorkspace>()
    for (const ws of all) {
        if (!ws.disabled) continue
        const key = rowKey(ws)
        if (!seen.has(key)) seen.set(key, ws)
    }
    return [...seen.values()]
}

/// How many top-level rows (repository groups and flat workspaces) are parked.
export function disabledRowCount(all: RegisteredWorkspace[]): number {
    return disabledRows(all).length
}

/// The other registered workspaces sharing `ws`'s row key — the sibling
/// worktrees of one repository, which therefore share its disabled state, its
/// display name and its tint. Always empty for a flat workspace, whose key is
/// its own path.
export function siblingsOf(
    ws: RegisteredWorkspace,
    all: RegisteredWorkspace[],
): RegisteredWorkspace[] {
    if (ws.repoId === null) return []
    const key = rowKey(ws)
    return all.filter((other) => other.uri !== ws.uri && rowKey(other) === key)
}

/// Every PARKED top-level row that a registry slug `token` could name, at most
/// one per row (`kind` narrows to one pool, as `matchSlug` does).
///
/// A parked row is absent from `views`, so its slug cannot be derived from one;
/// it is reconstructed from the listing instead, using exactly the inputs
/// `slug.ts` would have used — `slugify(name)` for the bare form and
/// `${base}-${shortHash(identity)}` for the suffixed one, where the identity is
/// the repository's `repoId` or the flat workspace's `uri`.
///
/// The reconstruction is exact for a flat workspace, whose registered `name`
/// IS the name a view carries. For a repository the view's name is its MAIN
/// worktree's basename, so this matches whenever the user registered the main
/// worktree (the ordinary case) and degrades to no match — i.e. to today's
/// not-found — when only a secondary worktree of the repository is registered.
/// It never yields a false match: an unrelated token matches nothing.
export function matchParkedSlug(
    token: string,
    registered: RegisteredWorkspace[],
    kind?: "flat" | "repo",
): RegisteredWorkspace[] {
    const matched = new Map<string, RegisteredWorkspace>()
    for (const ws of registered) {
        if (!ws.disabled) continue
        const rowKind = ws.repoId !== null ? "repo" : "flat"
        if (kind !== undefined && kind !== rowKind) continue
        const base = slugify(ws.name)
        if (token !== base && token !== `${base}-${shortHash(ws.repoId ?? ws.uri)}`) continue
        const key = rowKey(ws)
        if (!matched.has(key)) matched.set(key, ws)
    }
    return [...matched.values()]
}

/// What a today's-ships row can do when clicked. The Dashboard is deliberately
/// unfiltered (design.md D7: it is the ambient cue that parked work still
/// exists), so a parked repository's ships keep rendering — but the row must
/// say what it is instead of silently doing nothing.
export type ShipRowState =
    | { kind: "openable"; view: WorkspaceView }
    | { kind: "parked"; workspace: RegisteredWorkspace }
    | { kind: "unavailable" }

/// Which top-level row a ship belongs to, and whether that row can be opened.
///
/// Matching is by `repoId` — the identity the ship carries — never by
/// `worktreePath`: a change is routinely archived from inside a feature
/// worktree that hosts no ACTIVE change afterwards, and such a worktree is
/// neither a repo's `mainWorktree` nor any active instance's path, so a
/// path-keyed lookup misses it even when the repository is perfectly enabled.
export function shipRowState(
    entry: ShipEntry,
    views: WorkspaceView[],
    registered: RegisteredWorkspace[],
): ShipRowState {
    const view = views.find((v) => v.kind === "repo" && v.repoId === entry.repoId)
    if (view) return { kind: "openable", view }
    const parked = registered.find((w) => w.disabled && w.repoId === entry.repoId)
    return parked ? { kind: "parked", workspace: parked } : { kind: "unavailable" }
}

/// A row's human label — the configured display name when set, else the
/// registered name. Matches how the tree and Settings label the same row.
export function rowLabel(ws: RegisteredWorkspace): string {
    return ws.displayName ?? ws.name
}
