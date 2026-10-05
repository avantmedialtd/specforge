// Pure Address <-> URL-path codec — no DOM, no registered-workspace data, no
// backend call (`view-routing`: *Address and URL Round-Trip Through a Pure
// Codec*). Slug/instance tokens are opaque strings as far as this module is
// concerned; `slug.ts` decides what goes in them — this module only knows
// how to place a string in a path segment and read it back out.
//
// Grammar (see design.md):
//   /                                          home
//   /settings                                  settings, Workspaces group (the default)
//   /settings/<group>                          settings, one named group
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
//   /w/<workspace>/file/<path…>                one markdown file
//   /r/<repo>/file/<path…>                     one markdown file, main worktree
//   /pr/github/<owner>/<repo>/<number>         pull request, GitHub
//   /pr/bitbucket/<workspace>/<repo>/<id>      pull request, BitBucket
//
// `<artifact>` is one of "proposal" | "design" | "tasks"; a capability spec
// always spells out the literal "specs" segment before its `<cap>` token, so
// the codec can tell a spec address from a bare artifact one — and an
// instance segment from a bare-artifact one — from the closed, known-up-front
// vocabulary alone, with no outside data.
//
// `file` is RESERVED at the change-id position, and is what keeps that
// property true for file addresses. A relative path is not a closed
// vocabulary: `/r/<repo>/openspec/specs/web-ui/spec.md` would otherwise read
// as change `openspec`, the literal `specs`, capability `web-ui`, plus a
// trailing segment the grammar has no slot for. The cost is that a change
// directory named exactly `file` is not addressable — a documented
// reservation, taken deliberately in preference to a grammar that needs
// registry data to disambiguate.
//
// A settings address always ENCODES its group, so each group has exactly one
// path. The bare `/settings` still DECODES — to the default group — because it
// is the only settings link that existed before groups did. `<group>` is a
// closed vocabulary like `<artifact>`: an id the codec does not know decodes to
// unresolvable rather than to some other group. Whether the current host
// offers a group is NOT decided here; the codec is host-independent, and
// `effectiveSettingsGroup` handles that at render time.
//
// `pr` is closed the same way, and so is the provider word beneath it: only
// `github` and `bitbucket`, so a pull-request path decodes with no provider,
// snapshot or registry data (`view-routing`: *Pull-Request Addresses*). The
// owner and repository are one escaped segment each; the number has exactly
// one spelling — a positive decimal integer with no sign, no leading zero and
// no escape, no larger than a JavaScript number holds exactly — and is read
// from the raw segment, so `%34%32` is not a second spelling of 42. Every
// other shape beneath `pr` is unresolvable, never a partial address.

import { DEFAULT_SETTINGS_GROUP, isSettingsGroup } from "../settingsGroups"
import type { ArtifactReadKind } from "../types"
import type { Address, ArchiveSelection, Scope, Unresolvable } from "./address"
import { UNRESOLVABLE } from "./address"

const ARTIFACT_KEYWORDS = new Set<string>(["proposal", "design", "tasks"])

/// Reserved at the change-id position — see the grammar note above.
const FILE_KEYWORD = "file"

/// The top-level word for a pull request — see the grammar note above.
const PULL_REQUEST_KEYWORD = "pr"

const PULL_REQUEST_NUMBER = /^[1-9][0-9]*$/

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
            return `/settings/${seg(address.group)}`
        case "archive": {
            if (!address.selection) return "/archive"
            const base = `/archive/${seg(address.selection.workspace)}/${seg(address.selection.archiveDir)}`
            return address.selection.worktreeHint ? `${base}/${seg(address.selection.worktreeHint)}` : base
        }
        case "files":
            return `/${encodeScopePrefix(address.scope)}`
        case "file": {
            // Each segment is escaped independently so the separators stay
            // separators: encoding the whole path would escape its slashes and
            // collapse it into one opaque segment.
            const tail = address.path
                .split("/")
                .filter((s) => s.length > 0)
                .map(seg)
                .join("/")
            return `/${encodeScopePrefix(address.scope)}/${FILE_KEYWORD}/${tail}`
        }
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
        case "pullRequest": {
            const { provider, owner, repo, number } = address
            return `/${PULL_REQUEST_KEYWORD}/${provider}/${seg(owner)}/${seg(repo)}/${number}`
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
    if (parts[0] === "settings") return decodeSettings(parts)
    if (parts[0] === "archive") return decodeArchive(parts)
    if (parts[0] === "w") {
        return decodeScoped(parts, { kind: "workspace", workspace: unseg(parts[1] ?? "") })
    }
    if (parts[0] === "r") {
        return decodeScoped(parts, { kind: "repo", repo: unseg(parts[1] ?? "") })
    }
    if (parts[0] === PULL_REQUEST_KEYWORD) return decodePullRequest(parts)
    return UNRESOLVABLE
}

/// `/pr/<provider>/<owner>/<repo>/<number>`, exactly five segments.
function decodePullRequest(parts: string[]): Address | Unresolvable {
    if (parts.length !== 5) return UNRESOLVABLE
    const provider = parts[1]!
    if (provider !== "github" && provider !== "bitbucket") return UNRESOLVABLE
    const number = decodePullRequestNumber(parts[4]!)
    if (number === null) return UNRESOLVABLE
    return {
        kind: "pullRequest",
        provider,
        owner: unseg(parts[2]!),
        repo: unseg(parts[3]!),
        number,
    }
}

/// The number a raw segment spells, or `null` for any other spelling. The
/// pattern alone admits integers past 2^53, which a JavaScript number would
/// round onto a neighbour's — two paths, one pull request — so the safe range
/// is checked on the parsed value.
function decodePullRequestNumber(segment: string): number | null {
    if (!PULL_REQUEST_NUMBER.test(segment)) return null
    const number = Number(segment)
    return Number.isSafeInteger(number) ? number : null
}

function decodeSettings(parts: string[]): Address | Unresolvable {
    if (parts.length === 1) return { kind: "settings", group: DEFAULT_SETTINGS_GROUP }
    if (parts.length === 2) {
        const group = unseg(parts[1]!)
        if (isSettingsGroup(group)) return { kind: "settings", group }
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

    // The reserved `file` segment claims everything after it as one relative
    // path, so this is decided before any artifact shape is considered.
    if (parts[2] === FILE_KEYWORD) {
        const tail = parts.slice(3).map(unseg)
        // `/w/<slug>/file` with nothing after it names no document.
        if (tail.length === 0) return UNRESOLVABLE
        return { kind: "file", scope: base, path: tail.join("/") }
    }

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
