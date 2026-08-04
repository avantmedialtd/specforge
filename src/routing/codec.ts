// Pure Address <-> URL-path codec — no DOM, no registered-workspace data, no
// backend call (`view-routing`: *Address and URL Round-Trip Through a Pure
// Codec*). Slug/instance tokens are opaque strings as far as this module is
// concerned; `slug.ts` decides what goes in them — this module only knows
// how to place a string in a path segment and read it back out.
//
// Grammar (see design.md):
//   /                                          home
//   /settings                                  settings
//   /archive                                   archive (no selection)
//   /archive/<workspace>/<archive-dir>         archive (pre-selected)
//   /archive/<workspace>/<archive-dir>/<hint>  archive (pre-selected, exact worktree hint)
//   /w/<workspace>                             files, flat workspace
//   /w/<workspace>/<change>/<artifact>         artifact (flat workspace)
//   /w/<workspace>/<change>/specs/<cap>        spec (flat workspace)
//   /r/<repo>                                  files, repo main worktree
//   /r/<repo>/<change>/<artifact>              artifact, single-instance change
//   /r/<repo>/<change>/<instance>/<artifact>   artifact, multi-instance change
//   /r/<repo>/<change>/specs/<cap>             spec, single-instance change
//   /r/<repo>/<change>/<instance>/specs/<cap>  spec, multi-instance change
//
// `<artifact>` is one of "proposal" | "design" | "tasks"; a capability spec
// always spells out the literal "specs" segment before its `<cap>` token, so
// the codec can tell a spec address from a bare artifact one — and an
// instance segment from a bare-artifact one — from the closed, known-up-front
// vocabulary alone, with no outside data.

import type { ArtifactReadKind } from "../types"
import type { Address, ArchiveSelection, Scope, Unresolvable } from "./address"
import { UNRESOLVABLE } from "./address"

const ARTIFACT_KEYWORDS = new Set<string>(["proposal", "design", "tasks"])

function seg(value: string): string {
    return encodeURIComponent(value)
}

/// Decode one path segment. A malformed percent-escape falls back to the raw
/// token rather than throwing out of a pure function — an odd link should
/// decode to *something* (which will then simply fail to resolve to any
/// registered entity) rather than crash the caller.
function unseg(value: string): string {
    try {
        return decodeURIComponent(value)
    } catch {
        return value
    }
}

function pathSegments(path: string): string[] {
    return path.split("/").filter((s) => s.length > 0)
}

export function encodeAddress(address: Address): string {
    switch (address.kind) {
        case "home":
            return "/"
        case "settings":
            return "/settings"
        case "archive": {
            if (!address.selection) return "/archive"
            const base = `/archive/${seg(address.selection.workspace)}/${seg(address.selection.archiveDir)}`
            return address.selection.worktreeHint ? `${base}/${seg(address.selection.worktreeHint)}` : base
        }
        case "files":
            return `/${encodeScopePrefix(address.scope)}`
        case "artifact": {
            const prefix = encodeScopePrefix(address.scope)
            const change = seg(address.changeId)
            const instance =
                address.scope.kind === "repo" && address.scope.instance
                    ? `/${seg(address.scope.instance)}`
                    : ""
            const tail =
                address.artifactKind === "spec"
                    ? `specs/${seg(address.capability ?? "")}`
                    : address.artifactKind
            return `/${prefix}/${change}${instance}/${tail}`
        }
    }
}

/// `w/<slug>` or `r/<slug>` — the scope's own base segment, sans instance
/// (the instance segment, when present, is only ever appended by an
/// `artifact` address; `files` never carries one — a repo's file browser
/// always opens its main worktree).
function encodeScopePrefix(scope: Scope): string {
    return scope.kind === "workspace" ? `w/${seg(scope.workspace)}` : `r/${seg(scope.repo)}`
}

export function decodeAddress(path: string): Address | Unresolvable {
    const parts = pathSegments(path)

    if (parts.length === 0) return { kind: "home" }
    if (parts.length === 1 && parts[0] === "settings") return { kind: "settings" }
    if (parts[0] === "archive") return decodeArchive(parts)
    if (parts[0] === "w") {
        return decodeScoped(parts, { kind: "workspace", workspace: unseg(parts[1] ?? "") })
    }
    if (parts[0] === "r") {
        return decodeScoped(parts, { kind: "repo", repo: unseg(parts[1] ?? "") })
    }
    return UNRESOLVABLE
}

function decodeArchive(parts: string[]): Address | Unresolvable {
    if (parts.length === 1) return { kind: "archive", selection: null }
    if (parts.length === 3) {
        const selection: ArchiveSelection = {
            workspace: unseg(parts[1]!),
            archiveDir: unseg(parts[2]!),
        }
        return { kind: "archive", selection }
    }
    if (parts.length === 4) {
        const selection: ArchiveSelection = {
            workspace: unseg(parts[1]!),
            archiveDir: unseg(parts[2]!),
            worktreeHint: unseg(parts[3]!),
        }
        return { kind: "archive", selection }
    }
    return UNRESOLVABLE
}

/// Shared tail-parsing for `/w/...` and `/r/...`. `base` already carries the
/// scope classified by its `w`/`r` prefix (that's the one piece the codec
/// *can* determine without workspace data); this reads whatever follows the
/// slug segment (`parts[1]`, already folded into `base`).
function decodeScoped(parts: string[], base: Scope): Address | Unresolvable {
    if (parts.length < 2 || !parts[1]) return UNRESOLVABLE
    if (parts.length === 2) return { kind: "files", scope: base }
    if (parts.length < 4) return UNRESOLVABLE

    const changeId = unseg(parts[2]!)

    // .../<change>/<artifact>                     → 4 segments
    if (parts.length === 4) {
        const artifact = parts[3]!
        if (!ARTIFACT_KEYWORDS.has(artifact)) return UNRESOLVABLE
        return artifactAddress(base, changeId, artifact as ArtifactReadKind)
    }

    // .../<change>/specs/<cap>                    → 5 segments (specs form)
    // .../<change>/<instance>/<artifact>          → 5 segments (instance form)
    if (parts.length === 5) {
        if (parts[3] === "specs") {
            return artifactAddress(base, changeId, "spec", unseg(parts[4]!))
        }
        // Flat workspaces have no instance concept at all.
        if (base.kind !== "repo") return UNRESOLVABLE
        const artifact = parts[4]!
        if (!ARTIFACT_KEYWORDS.has(artifact)) return UNRESOLVABLE
        return artifactAddress(
            { ...base, instance: unseg(parts[3]!) },
            changeId,
            artifact as ArtifactReadKind,
        )
    }

    // .../<change>/<instance>/specs/<cap>         → 6 segments
    if (parts.length === 6 && base.kind === "repo" && parts[4] === "specs") {
        return artifactAddress(
            { ...base, instance: unseg(parts[3]!) },
            changeId,
            "spec",
            unseg(parts[5]!),
        )
    }

    return UNRESOLVABLE
}

function artifactAddress(
    scope: Scope,
    changeId: string,
    artifactKind: ArtifactReadKind,
    capability?: string,
): Address {
    return {
        kind: "artifact",
        scope,
        changeId,
        artifactKind,
        ...(capability !== undefined ? { capability } : {}),
    }
}
