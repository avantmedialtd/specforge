// Registry-slug identity: derives a URL-safe slug from a workspace's or
// repository's STABLE registered name (never the mutable `displayName`
// override), assigns the shortest form that resolves uniquely against the
// currently loaded `WorkspaceView[]`, and matches a token back to the
// view(s) it could name (`view-routing`: *Workspace Identity Is a Registry
// Slug*, *Shortest Unambiguous Address*).

import type { ChangeInstance, WorkspaceView } from "../types"
import type { Scope } from "./address"

/// Lowercase alphanumeric runs joined by single hyphens, trimmed — never
/// itself a filesystem path (non-alphanumeric characters, including `/`, are
/// stripped), so an Address built from it can never carry one (*An address
/// never contains a host path*). Falls back to a fixed placeholder for a
/// name that slugifies to nothing (all-punctuation / all-emoji), so a path
/// segment is never empty.
export function slugify(name: string): string {
    const base = name
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "")
    return base.length > 0 ? base : "x"
}

/// A short, deterministic, non-reversible token (FNV-1a 32-bit, base36) used
/// only to disambiguate a colliding slug or to name a worktree instance.
/// Hashing (rather than e.g. a parent-directory segment) keeps host
/// directory layout unpublished even in the disambiguating case.
export function shortHash(identity: string): string {
    let hash = 0x811c9dc5
    for (let i = 0; i < identity.length; i++) {
        hash ^= identity.charCodeAt(i)
        hash = Math.imul(hash, 0x01000193)
    }
    return (hash >>> 0).toString(36)
}

function stableName(view: WorkspaceView): string {
    return view.kind === "repo" ? view.name : view.workspace.name
}

/// The identity a colliding slug's suffix is hashed from — never a
/// displayName, always the value that also identifies the entity to the
/// backend (so it changes only if the registration itself changes).
function stableIdentity(view: WorkspaceView): string {
    return view.kind === "repo" ? view.repoId : view.workspace.uri
}

function baseSlug(view: WorkspaceView): string {
    return slugify(stableName(view))
}

function suffixedSlug(view: WorkspaceView): string {
    return `${baseSlug(view)}-${shortHash(stableIdentity(view))}`
}

/// The shortest slug for `view` that is unique against every OTHER view of
/// the SAME kind currently registered — a bare slug, or (only on collision)
/// the base plus a hash suffix. Flat workspaces and repos are collision-
/// checked independently: the `/w/`/`/r/` prefix already keeps the two kinds
/// from being confused when decoding, so a workspace and a repo sharing a
/// name never force each other into a suffix.
export function slugFor(view: WorkspaceView, views: WorkspaceView[]): string {
    const base = baseSlug(view)
    const collides = views.some(
        (other) =>
            stableIdentity(other) !== stableIdentity(view) &&
            other.kind === view.kind &&
            baseSlug(other) === base,
    )
    return collides ? suffixedSlug(view) : base
}

/// Every currently-registered view — of `kind` if given, else both flat
/// workspaces and repos together — that `token` could name: a match on its
/// bare base slug (regardless of whether that view currently needs a
/// suffix), or an exact match on its fully-suffixed form. Zero means the
/// token names nothing; more than one means it is ambiguous (*Shortest
/// Unambiguous Address*) — either a colliding slug was registered, or the
/// token was emitted before a collision existed and is now ambiguous without
/// a suffix it never had. Checking the bare base (not just each view's OWN
/// current `slugFor`) is what makes that "became ambiguous" case detectable:
/// once two views collide, NEITHER emits the bare form any more, so an old
/// bare link must still find both to report the ambiguity rather than
/// silently reporting not-found.
export function matchSlug(
    token: string,
    views: WorkspaceView[],
    kind?: "flat" | "repo",
): WorkspaceView[] {
    const pool = kind ? views.filter((v) => v.kind === kind) : views
    return pool.filter((v) => baseSlug(v) === token || suffixedSlug(v) === token)
}

/// The `Scope` addressing `view` on its own — never carries an instance (see
/// `instanceToken` for that, used only inside an `artifact` address).
export function scopeFor(view: WorkspaceView, views: WorkspaceView[]): Scope {
    const slug = slugFor(view, views)
    return view.kind === "repo" ? { kind: "repo", repo: slug } : { kind: "workspace", workspace: slug }
}

/// The instance segment for `worktreePath` within a logical change's
/// `instances` — `undefined` when the change has only one instance, so a
/// single-instance address never carries the segment at all (*A
/// single-instance change omits the instance segment*).
export function instanceToken(
    worktreePath: string,
    instances: ChangeInstance[],
): string | undefined {
    return instances.length > 1 ? shortHash(worktreePath) : undefined
}

/// The instance within `instances` whose token equals `token`, or
/// `undefined` when the token no longer names any current instance (the
/// worktree it pointed at is gone).
export function matchInstance(
    token: string,
    instances: ChangeInstance[],
): ChangeInstance | undefined {
    return instances.find((inst) => shortHash(inst.worktreePath) === token)
}
