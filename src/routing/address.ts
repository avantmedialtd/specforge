// Addressable viewing state — the identifier-only value that names what the
// center pane (or the settings / archive overlay) currently shows. See the
// `view-routing` capability spec's *Addressable Viewing State* requirement.
//
// An Address carries only stable identifiers (registry slugs, change ids,
// artifact kinds) — never a resolved payload, a derived display label, or an
// absolute filesystem path. It means nothing on its own; `resolve.ts` turns
// one into a render target against the currently loaded `WorkspaceView[]`.

import type { ArtifactReadKind } from "../types"

/// Identifies which registered entity a `files`/`artifact` address's scope
/// names: a flat (non-git) workspace by its own registry slug, or a
/// repository by its slug — optionally naming one of its worktree instances,
/// which only a multi-instance change ever needs (*Shortest Unambiguous
/// Address*). Slugs are opaque tokens assigned by `slug.ts`; nothing here is
/// or contains a filesystem path.
export type Scope =
    | { kind: "workspace"; workspace: string }
    | { kind: "repo"; repo: string; instance?: string }

/// A pending archive pre-selection: which registered entity's archive to
/// browse, and the on-disk archive directory name (`<YYYY-MM-DD>-<id>` or a
/// legacy bare id) to open within it. `workspace` never carries a repo
/// `instance`-shaped segment the way an artifact `Scope` can — archived
/// changes carry no per-worktree distinction in the data the frontend reads
/// (`ArchivedChangeSummary`); `worktreeHint` (below) is a narrower, opt-in
/// mechanism for the one case that does need worktree precision.
export interface ArchiveSelection {
    /// Registry slug — resolved against BOTH the flat-workspace and repo
    /// pools by `resolve.ts`, since the `/archive/<workspace>/...` grammar
    /// carries no `w`/`r` prefix to pre-classify it (unlike `files`/
    /// `artifact`), and the codec must decode without workspace data.
    workspace: string
    archiveDir: string
    /// A hash of the exact worktree (see `shortHash`) the address was formed
    /// from, for a repo-scoped selection — never the path itself (*An
    /// address never contains a host path*). `openspec/changes/archive/` is
    /// git-tracked, so a worktree whose checkout hasn't yet merged the
    /// archival commit that just happened in a DIFFERENT worktree of the
    /// same repo (a routine state for a repo archived from inside its own
    /// feature worktrees, per this project's own workflow) would otherwise
    /// silently show an archive listing that doesn't contain the very
    /// change the link named. Re-verified at resolution against the repo's
    /// currently active instances; when none match — the worktree no longer
    /// hosts anything active, e.g. it was a throwaway worktree removed after
    /// merging — resolution falls back to the repo's main worktree rather
    /// than failing outright. Absent for a flat-workspace selection, which
    /// has no worktree concept, and absent when the address was formed
    /// without a known worktree to hint (e.g. picking an archive
    /// disambiguation candidate).
    worktreeHint?: string
}

export type Address =
    | { kind: "home" }
    | { kind: "settings" }
    | { kind: "archive"; selection: ArchiveSelection | null }
    | { kind: "files"; scope: Scope }
    | {
          kind: "artifact"
          scope: Scope
          changeId: string
          artifactKind: ArtifactReadKind
          /// Present iff `artifactKind === "spec"`.
          capability?: string
      }

/// The outcome of decoding a path that does not match the Address grammar —
/// never a partially-populated `Address` (*Address and URL Round-Trip
/// Through a Pure Codec*). Carries a `kind` discriminant like every `Address`
/// variant, so callers can switch on one union covering both.
export interface Unresolvable {
    kind: "unresolvable"
}

export const UNRESOLVABLE: Unresolvable = { kind: "unresolvable" }
