// Resolves an Address against the currently loaded WorkspaceView[] into
// exactly what the shell needs to render — or reports why it can't
// (`view-routing`: *Cold-Load Address Resolution*, *Shortest Unambiguous
// Address*). The reverse mapping, `renderTargetToAddress`, lets a tree/rail
// click publish the same Address this module would resolve back to it.
//
// Resolution never reads an artifact or lists a directory for an unresolved
// slug — every outcome below is computed purely from the already-loaded
// `WorkspaceView[]` (an *Unknown slug reads nothing* — the same closed-set
// guarantee the `view-routing` capability's *Workspace Identity Is a
// Registry Slug* requirement makes for slugs generally).

import type {
    ArtifactReadKind,
    ArtifactRenderTarget,
    ChangeData,
    ChangeInstance,
    FilesRenderTarget,
    RegisteredWorkspace,
    RenderTarget,
    WorkspaceView,
} from "../types"
import { matchParkedSlug } from "../workspaceRows"
import type { Address, ArchiveSelection, Scope } from "./address"
import { archiveSlugFor, instanceToken, matchInstance, matchSlug, scopeFor, shortHash } from "./slug"

/// What the resolved address means for the shell to render — narrower than
/// `RenderTarget` because it also covers the settings/archive overlay panes,
/// which are not render targets at all (there is no `centerTarget` value for
/// "settings is open" today, and this change does not invent one).
export type ResolvedView =
    | { kind: "home" }
    | { kind: "settings" }
    | { kind: "archive"; selection: ResolvedArchiveSelection | null }
    | { kind: "target"; target: RenderTarget }

export interface ResolvedArchiveSelection {
    workspaceUri: string
    archiveDir: string
}

/// One candidate an ambiguous address could mean — a human-presentable label
/// plus the fully-disambiguated Address that names exactly it.
export interface Candidate {
    label: string
    address: Address
}

export type ResolveResult =
    | { status: "resolved"; view: ResolvedView }
    | { status: "ambiguous"; candidates: Candidate[] }
    /// The address names a workspace that IS registered but is disabled, so it
    /// is absent from `views` and has nothing to open — a distinct outcome from
    /// not-found, because parking is reversible and the row is still there
    /// (`view-routing`: *Cold-Load Address Resolution*). At most one entry per
    /// parked top-level row.
    | { status: "disabled"; workspaces: RegisteredWorkspace[] }
    | { status: "notFound" }

const RESOLVED_HOME: ResolveResult = { status: "resolved", view: { kind: "home" } }
const RESOLVED_SETTINGS: ResolveResult = { status: "resolved", view: { kind: "settings" } }
const NOT_FOUND: ResolveResult = { status: "notFound" }

/// `registered` is the UNFILTERED registered listing (`list_workspaces`),
/// consulted only when a workspace token matches no view: it is the sole
/// source that still carries a parked row, so it is what tells "this link's
/// workspace is disabled" apart from "this link names nothing at all". It
/// defaults to empty for callers that only need the resolved/not-resolved
/// distinction (`addressToNodePath`, which reveals nothing for either).
export function resolveAddress(
    address: Address,
    views: WorkspaceView[],
    registered: RegisteredWorkspace[] = [],
): ResolveResult {
    switch (address.kind) {
        case "home":
            return RESOLVED_HOME
        case "settings":
            return RESOLVED_SETTINGS
        case "archive":
            return resolveArchive(address.selection, views, registered)
        case "files":
            return resolveFiles(address.scope, views, registered)
        case "file":
            return resolveFiles(address.scope, views, registered, address.path)
        case "artifact":
            return resolveArtifact(address, views, registered)
    }
}

/// The parked rows `token` could name, as a `disabled` result — or `null` when
/// it names none, leaving the caller's not-found outcome intact. Consulted ONLY
/// where a workspace/repo token matched no view: a change or artifact missing
/// *inside* a resolvable workspace is a genuine miss, not a parked row.
function parkedResult(
    token: string,
    registered: RegisteredWorkspace[],
    kind?: "flat" | "repo",
): ResolveResult | null {
    const parked = matchParkedSlug(token, registered, kind)
    return parked.length > 0 ? { status: "disabled", workspaces: parked } : null
}

// ---- Archive ---------------------------------------------------------

function resolveArchive(
    selection: ArchiveSelection | null,
    views: WorkspaceView[],
    registered: RegisteredWorkspace[],
): ResolveResult {
    if (!selection) {
        return { status: "resolved", view: { kind: "archive", selection: null } }
    }
    // Both pools together, deliberately — see the *archive-style cross-pool
    // lookup* case in `slug.test.ts` and `archiveSlugFor`'s own doc comment.
    // The parked lookup passes no `kind` for the same reason.
    const matches = matchSlug(selection.workspace, views)
    if (matches.length === 0) {
        return parkedResult(selection.workspace, registered) ?? NOT_FOUND
    }
    if (matches.length > 1) {
        return {
            status: "ambiguous",
            candidates: matches.map((v) => ({
                label: candidateLabel(v),
                address: {
                    kind: "archive",
                    selection: {
                        // C1: `archiveSlugFor`, not `scopeFor`/`slugFor` —
                        // those de-duplicate per-kind, so a same-named flat
                        // workspace and repo would each re-encode to the
                        // identical bare slug and the chooser would never
                        // resolve. `worktreeHint` carries forward
                        // unconditionally, same reasoning as B3's
                        // `carryInstance`: harmless if the picked candidate
                        // turns out to be the wrong one (it just won't match
                        // there, and resolution degrades to that
                        // candidate's main worktree).
                        workspace: archiveSlugFor(v, views),
                        archiveDir: selection.archiveDir,
                        worktreeHint: selection.worktreeHint,
                    },
                },
            })),
        }
    }
    const view = matches[0]!
    const workspaceUri =
        view.kind === "repo"
            ? worktreeForHint(view, selection.worktreeHint, registered) ?? view.mainWorktree
            : view.workspace.uri
    return {
        status: "resolved",
        view: { kind: "archive", selection: { workspaceUri, archiveDir: selection.archiveDir } },
    }
}

/// The worktree of `view`'s repository that `hint` names — searched over the
/// repository's TRACKED WORKTREES, plus its active instances and its registered
/// folders as belt-and-braces. `null` when `hint` is absent or names none,
/// leaving the caller's fallback to the repo's main worktree.
///
/// `RepoView.worktrees` is the load-bearing pool, and the reason is the shape of
/// the case the hint exists for. A `worktreeHint` is minted by the today's-ships
/// feed for the worktree a change was ARCHIVED from, and such a worktree
/// routinely (a) hosts no active change afterwards, so it appears in no
/// `view.active` instance — `RepoView.archived` is never serialized
/// (`repo_view.rs`'s `skip_serializing`) — and (b) was AUTO-DISCOVERED rather
/// than registered by the user, so it appears in no `list_workspaces` row
/// either. Both older pools therefore miss precisely the worktree that holds
/// the change, and the fallback would open the MAIN worktree.
///
/// Since the Archive view now lists a repository's archived changes across all
/// of its tracked worktrees, a miss here no longer loses the change — it only
/// picks a different copy to open first. The pools are kept anyway because they
/// cost nothing and a hit is the correct copy.
///
/// Every pass is restricted to this repository: `shortHash` is a 32-bit token
/// over a bare path with no repository in it, so an unrestricted scan could
/// hand back a wholly unrelated repository's worktree on a collision.
function worktreeForHint(
    view: Extract<WorkspaceView, { kind: "repo" }>,
    hint: string | undefined,
    registered: RegisteredWorkspace[],
): string | null {
    if (!hint) return null
    return (
        view.worktrees.find((wt) => shortHash(wt) === hint) ??
        findActiveWorktreeByHash(view, hint) ??
        findRegisteredWorktreeByHash(view.repoId, hint, registered)
    )
}

/// The worktree path among `view`'s CURRENTLY active instances whose hash
/// equals `hint`, or `null`.
function findActiveWorktreeByHash(
    view: Extract<WorkspaceView, { kind: "repo" }>,
    hint: string,
): string | null {
    for (const lc of view.active) {
        for (const inst of lc.instances) {
            if (shortHash(inst.worktreePath) === hint) return inst.worktreePath
        }
    }
    return null
}

/// The registered folder OF THIS REPOSITORY whose path hashes to `hint`, or
/// `null` (C2: the worktree the hint named has since been unregistered or
/// removed — the caller falls back to the repo's main worktree, which is not
/// always correct but is the best available guess with no backend read).
function findRegisteredWorktreeByHash(
    repoId: string,
    hint: string,
    registered: RegisteredWorkspace[],
): string | null {
    const match = registered.find((ws) => ws.repoId === repoId && shortHash(ws.uri) === hint)
    return match ? match.uri : null
}

// ---- Files ---------------------------------------------------------------

/// Resolves both the `files` address (browse root only) and the `file`
/// address (browse root plus a selected document) — they differ by exactly
/// `selectedPath`, and a file address is a `files` address that also names
/// which file, so resolving them separately would duplicate the scope
/// matching, the ambiguity fan-out and the parked-workspace fallback.
function resolveFiles(
    scope: Scope,
    views: WorkspaceView[],
    registered: RegisteredWorkspace[],
    selectedPath?: string,
): ResolveResult {
    const matches = matchScope(scope, views)
    if (matches.length === 0) return parkedScope(scope, registered) ?? NOT_FOUND
    if (matches.length > 1) {
        return {
            status: "ambiguous",
            candidates: matches.map((v) => ({
                label: candidateLabel(v),
                address:
                    selectedPath === undefined
                        ? { kind: "files", scope: scopeFor(v, views) }
                        : { kind: "file", scope: scopeFor(v, views), path: selectedPath },
            })),
        }
    }
    const view = matches[0]!
    const target: FilesRenderTarget = {
        kind: "files",
        root: view.kind === "repo" ? view.mainWorktree : view.workspace.uri,
        ...(selectedPath !== undefined ? { selectedPath } : {}),
    }
    return { status: "resolved", view: { kind: "target", target } }
}

// ---- Artifact --------------------------------------------------------

function resolveArtifact(
    address: Extract<Address, { kind: "artifact" }>,
    views: WorkspaceView[],
    registered: RegisteredWorkspace[],
): ResolveResult {
    const matches = matchScope(address.scope, views)
    if (matches.length === 0) return parkedScope(address.scope, registered) ?? NOT_FOUND
    if (matches.length > 1) {
        return {
            status: "ambiguous",
            candidates: matches.map((v) => ({
                label: candidateLabel(v),
                address: { ...address, scope: carryInstance(address.scope, scopeFor(v, views)) },
            })),
        }
    }
    const view = matches[0]!
    return view.kind === "repo" ? resolveRepoArtifact(view, address) : resolveFlatArtifact(view, address)
}

/// Preserve an ORIGINAL repo-scoped instance token onto a freshly-resolved
/// scope candidate (B3: a scope-level collision — e.g. a second repo
/// registered under the same slug — must not also discard an instance the
/// original address already named exactly; `scopeFor` never emits one on
/// its own, so a candidate rebuilt only from it silently forces a second,
/// spurious instance chooser for a worktree the link had already picked).
/// If the picked candidate isn't actually the repo the instance token was
/// computed against, `matchInstance` simply won't find it there and
/// resolution reports not-found — an honest outcome for having picked the
/// wrong scope candidate, not a second guess.
function carryInstance(original: Scope, resolved: Scope): Scope {
    return original.kind === "repo" && original.instance !== undefined && resolved.kind === "repo"
        ? { ...resolved, instance: original.instance }
        : resolved
}

function resolveFlatArtifact(
    view: Extract<WorkspaceView, { kind: "flat" }>,
    address: Extract<Address, { kind: "artifact" }>,
): ResolveResult {
    const change = view.changes.find((c) => c.changeId === address.changeId)
    if (!change) return NOT_FOUND
    if (!artifactPresent(change, address.artifactKind, address.capability)) return NOT_FOUND
    return targetResult({
        kind: "artifact",
        workspace: view.workspace.uri,
        changeId: change.changeId,
        artifactKind: address.artifactKind,
        capability: address.capability,
    })
}

function resolveRepoArtifact(
    view: Extract<WorkspaceView, { kind: "repo" }>,
    address: Extract<Address, { kind: "artifact" }>,
): ResolveResult {
    const lc = view.active.find((l) => l.name === address.changeId)
    if (!lc) return NOT_FOUND

    const scope = address.scope
    const instanceTok = scope.kind === "repo" ? scope.instance : undefined

    if (instanceTok === undefined) {
        if (lc.instances.length === 0) return NOT_FOUND
        if (lc.instances.length === 1) {
            return resolveInstance(lc.instances[0]!, address)
        }
        // No instance named, but more than one now exists — ambiguous (*A
        // multi-instance change names its instance* / *An address that has
        // become ambiguous presents a choice*), one candidate per instance.
        return {
            status: "ambiguous",
            candidates: lc.instances.map((inst) => ({
                label: instanceLabel(inst),
                address: {
                    ...address,
                    scope: { ...scope, instance: instanceToken(inst.worktreePath, lc.instances) },
                },
            })),
        }
    }

    const inst = matchInstance(instanceTok, lc.instances)
    if (!inst) return NOT_FOUND
    return resolveInstance(inst, address)
}

function resolveInstance(
    inst: ChangeInstance,
    address: Extract<Address, { kind: "artifact" }>,
): ResolveResult {
    if (!artifactPresent(inst.change, address.artifactKind, address.capability)) return NOT_FOUND
    return targetResult({
        kind: "artifact",
        workspace: inst.worktreePath,
        changeId: inst.change.changeId,
        artifactKind: address.artifactKind,
        capability: address.capability,
    })
}

function targetResult(target: ArtifactRenderTarget): ResolveResult {
    return { status: "resolved", view: { kind: "target", target } }
}

/// Whether `change` actually has the artifact `kind` (`capability` too, for
/// a spec) on disk — an address naming a present workspace/change but a
/// missing artifact is treated as not-found rather than handed to the detail
/// pane to fail loudly on.
function artifactPresent(change: ChangeData, kind: ArtifactReadKind, capability?: string): boolean {
    switch (kind) {
        case "proposal":
            return change.artifacts.proposal
        case "design":
            return change.artifacts.design
        case "tasks":
            return change.artifacts.tasks
        case "spec":
            return capability !== undefined && change.artifacts.specs.includes(capability)
    }
}

// ---- Shared helpers --------------------------------------------------

function matchScope(scope: Scope, views: WorkspaceView[]): WorkspaceView[] {
    return scope.kind === "workspace"
        ? matchSlug(scope.workspace, views, "flat")
        : matchSlug(scope.repo, views, "repo")
}

/// `parkedResult` for a scope — same pool split as `matchScope`, so a `/w/`
/// token never resolves to a parked repository or vice versa.
function parkedScope(scope: Scope, registered: RegisteredWorkspace[]): ResolveResult | null {
    return scope.kind === "workspace"
        ? parkedResult(scope.workspace, registered, "flat")
        : parkedResult(scope.repo, registered, "repo")
}

function candidateLabel(view: WorkspaceView): string {
    const name = view.kind === "repo" ? (view.displayName ?? view.name) : (view.displayName ?? view.workspace.name)
    const detail = view.kind === "repo" ? view.mainWorktree : view.workspace.uri
    return `${name} (${detail})`
}

function instanceLabel(inst: ChangeInstance): string {
    return inst.branch ?? basename(inst.worktreePath) ?? inst.worktreePath
}

function basename(path: string): string | null {
    const parts = path.split("/").filter(Boolean)
    return parts.length > 0 ? parts[parts.length - 1]! : null
}

// ---- Reverse mapping: click -> Address --------------------------------

/// The Address a tree/rail selection's RenderTarget would resolve back to —
/// `null` for a target that has none (`CommitRenderTarget`; see design.md's
/// *Normalize FilesRenderTarget now, leave CommitRenderTarget alone*, and the
/// `view-routing` *Addressable Viewing State* requirement's carve-out for
/// nodes that render nothing of their own).
export function renderTargetToAddress(target: RenderTarget, views: WorkspaceView[]): Address | null {
    switch (target.kind) {
        case "dashboard":
            return { kind: "home" }
        case "commit":
            return null
        case "files": {
            const view = findViewByRoot(target.root, views)
            if (!view) return null
            const scope = scopeFor(view, views)
            return target.selectedPath === undefined
                ? { kind: "files", scope }
                : { kind: "file", scope, path: target.selectedPath }
        }
        case "artifact": {
            const found = findWorkspaceMatch(target.workspace, views, target.changeId)
            if (!found) return null
            const baseScope = scopeFor(found.view, views)
            const scope: Scope =
                baseScope.kind === "repo" && found.instances && found.instances.length > 1
                    ? { ...baseScope, instance: instanceToken(target.workspace, found.instances) }
                    : baseScope
            return {
                kind: "artifact",
                scope,
                changeId: target.changeId,
                artifactKind: target.artifactKind,
                ...(target.capability !== undefined ? { capability: target.capability } : {}),
            }
        }
    }
}

export interface WorkspaceMatch {
    view: WorkspaceView
    /// Populated only for a repo-hosted match: the owning logical change's
    /// name (`instanceId`'s middle argument in `WorkspaceTree.tsx`) and its
    /// sibling instances, for `instanceToken` to disambiguate against.
    logicalChangeName?: string
    instances?: ChangeInstance[]
}

/// The view whose flat-workspace uri or repo main-worktree equals `root` —
/// what a `files` RenderTarget's `root` (or a `files`/`archive` Address's
/// resolved scope) points at.
export function findViewByRoot(root: string, views: WorkspaceView[]): WorkspaceView | null {
    for (const view of views) {
        if (view.kind === "flat" && view.workspace.uri === root) return view
        if (view.kind === "repo" && view.mainWorktree === root) return view
    }
    return null
}

/// The view (and, for a repo, the owning logical change) whose worktree
/// equals `workspaceUri` — a flat workspace's own uri, or one instance's
/// `worktreePath` among a repo's active changes.
///
/// `changeId`, when given, restricts the search to the logical change named
/// by it — required whenever the caller actually needs the right one back.
/// A single worktree can simultaneously host more than one active change
/// (e.g. two different `openspec/changes/<id>/` directories both checked
/// out in the same main worktree, with no separate worktree for either) —
/// without `changeId`, this would silently return whichever logical change
/// happens to be first, which is only safe when the caller (like
/// `repoIdForTarget`) only reads `.view` from the result, never
/// `.logicalChangeName`/`.instances` (B2: a caller that DOES read those,
/// resolved for one change, previously got another's, producing a node id
/// that matches no real row and a reveal that force-opens the wrong
/// change's subtree entirely).
export function findWorkspaceMatch(
    workspaceUri: string,
    views: WorkspaceView[],
    changeId?: string,
): WorkspaceMatch | null {
    for (const view of views) {
        if (view.kind === "flat" && view.workspace.uri === workspaceUri) {
            return { view }
        }
        if (view.kind === "repo") {
            for (const lc of view.active) {
                if (changeId !== undefined && lc.name !== changeId) continue
                for (const inst of lc.instances) {
                    if (inst.worktreePath === workspaceUri) {
                        return { view, logicalChangeName: lc.name, instances: lc.instances }
                    }
                }
            }
        }
    }
    return null
}
