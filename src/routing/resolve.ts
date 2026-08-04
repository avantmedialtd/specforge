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
    RenderTarget,
    WorkspaceView,
} from "../types"
import type { Address, ArchiveSelection, Scope } from "./address"
import { instanceToken, matchInstance, matchSlug, scopeFor } from "./slug"

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
    | { status: "notFound" }

const RESOLVED_HOME: ResolveResult = { status: "resolved", view: { kind: "home" } }
const RESOLVED_SETTINGS: ResolveResult = { status: "resolved", view: { kind: "settings" } }
const NOT_FOUND: ResolveResult = { status: "notFound" }

export function resolveAddress(address: Address, views: WorkspaceView[]): ResolveResult {
    switch (address.kind) {
        case "home":
            return RESOLVED_HOME
        case "settings":
            return RESOLVED_SETTINGS
        case "archive":
            return resolveArchive(address.selection, views)
        case "files":
            return resolveFiles(address.scope, views)
        case "artifact":
            return resolveArtifact(address, views)
    }
}

// ---- Archive ---------------------------------------------------------

function resolveArchive(
    selection: ArchiveSelection | null,
    views: WorkspaceView[],
): ResolveResult {
    if (!selection) {
        return { status: "resolved", view: { kind: "archive", selection: null } }
    }
    const matches = matchSlug(selection.workspace, views)
    if (matches.length === 0) return NOT_FOUND
    if (matches.length > 1) {
        return {
            status: "ambiguous",
            candidates: matches.map((v) => ({
                label: candidateLabel(v),
                address: {
                    kind: "archive",
                    selection: {
                        workspace: scopeToken(scopeFor(v, views)),
                        archiveDir: selection.archiveDir,
                    },
                },
            })),
        }
    }
    const view = matches[0]!
    const workspaceUri = view.kind === "repo" ? view.mainWorktree : view.workspace.uri
    return {
        status: "resolved",
        view: { kind: "archive", selection: { workspaceUri, archiveDir: selection.archiveDir } },
    }
}

// ---- Files ---------------------------------------------------------------

function resolveFiles(scope: Scope, views: WorkspaceView[]): ResolveResult {
    const matches = matchScope(scope, views)
    if (matches.length === 0) return NOT_FOUND
    if (matches.length > 1) {
        return {
            status: "ambiguous",
            candidates: matches.map((v) => ({
                label: candidateLabel(v),
                address: { kind: "files", scope: scopeFor(v, views) },
            })),
        }
    }
    const view = matches[0]!
    const target: FilesRenderTarget = {
        kind: "files",
        root: view.kind === "repo" ? view.mainWorktree : view.workspace.uri,
    }
    return { status: "resolved", view: { kind: "target", target } }
}

// ---- Artifact --------------------------------------------------------

function resolveArtifact(
    address: Extract<Address, { kind: "artifact" }>,
    views: WorkspaceView[],
): ResolveResult {
    const matches = matchScope(address.scope, views)
    if (matches.length === 0) return NOT_FOUND
    if (matches.length > 1) {
        return {
            status: "ambiguous",
            candidates: matches.map((v) => ({
                label: candidateLabel(v),
                address: { ...address, scope: scopeFor(v, views) },
            })),
        }
    }
    const view = matches[0]!
    return view.kind === "repo" ? resolveRepoArtifact(view, address) : resolveFlatArtifact(view, address)
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

function scopeToken(scope: Scope): string {
    return scope.kind === "workspace" ? scope.workspace : scope.repo
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
            return view ? { kind: "files", scope: scopeFor(view, views) } : null
        }
        case "artifact": {
            const found = findWorkspaceMatch(target.workspace, views)
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
export function findWorkspaceMatch(workspaceUri: string, views: WorkspaceView[]): WorkspaceMatch | null {
    for (const view of views) {
        if (view.kind === "flat" && view.workspace.uri === workspaceUri) {
            return { view }
        }
        if (view.kind === "repo") {
            for (const lc of view.active) {
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
