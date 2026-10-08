/// The zoom window's address (`diff-view`: *Image Comparison*, Zooming;
/// design D11): what names an image file's two versions, by reference and
/// never by bytes, so the window can read them again itself. One canonical
/// string per file, which the desktop's window label and the browser tab's
/// name are both hashed from, so asking twice for one file finds its window.
/// Pure, for the reason `diffLayout.ts` gives in its header.

import { shortHash } from "./routing/slug"
import type { PullRequestProvider, PullRequestReference } from "./types"

/// The two sides' names, as the diff the window was opened from shows them.
export interface SideNames {
    old: string
    new: string
}

/// What names an image file's versions: a commit's file, or a pull
/// request's.
export type ImageWindowSource =
    | {
          kind: "commit"
          repoId: string
          sha: string
          path: string
          oldPath: string | null
          sides: SideNames
      }
    | {
          kind: "pullRequest"
          reference: PullRequestReference
          path: string
          head: string
          base: string
          sides: SideNames
      }

/// The source's address: its fields in one fixed order, so one file has one
/// address however the source was built.
export function imageWindowAddress(source: ImageWindowSource): string {
    const sides = { old: source.sides.old, new: source.sides.new }
    switch (source.kind) {
        case "commit":
            return JSON.stringify({
                kind: "commit",
                repoId: source.repoId,
                sha: source.sha,
                path: source.path,
                oldPath: source.oldPath,
                sides,
            })
        case "pullRequest":
            return JSON.stringify({
                kind: "pullRequest",
                reference: {
                    provider: source.reference.provider,
                    owner: source.reference.owner,
                    repo: source.reference.repo,
                    number: source.reference.number,
                },
                path: source.path,
                head: source.head,
                base: source.base,
                sides,
            })
    }
}

const PROVIDERS: readonly PullRequestProvider[] = ["github", "bitbucket"]

type Json = Record<string, unknown>

const isText = (value: unknown): value is string => typeof value === "string"
const isObject = (value: unknown): value is Json =>
    typeof value === "object" && value !== null && !Array.isArray(value)

/// The source an address names, or null for anything that is not one: the
/// window was opened from a URL, so nothing about it is taken on trust, and
/// the service checks the rest again.
export function parseImageWindowAddress(address: string | null): ImageWindowSource | null {
    if (address === null) return null
    let value: unknown
    try {
        value = JSON.parse(address)
    } catch {
        return null
    }
    if (!isObject(value) || !isObject(value.sides)) return null
    const { old, new: next } = value.sides
    if (!isText(old) || !isText(next) || !isText(value.path) || value.path === "") return null
    const sides = { old, new: next }
    if (value.kind === "commit") {
        const { repoId, sha, oldPath } = value
        if (!isText(repoId) || repoId === "" || !isText(sha) || !/^[0-9a-f]{4,64}$/i.test(sha)) {
            return null
        }
        if (oldPath !== null && (!isText(oldPath) || oldPath === "")) return null
        return { kind: "commit", repoId, sha, path: value.path, oldPath, sides }
    }
    if (value.kind === "pullRequest") {
        const { reference, head, base } = value
        if (!isObject(reference) || !isText(head) || !isText(base)) return null
        const { provider, owner, repo, number } = reference
        if (
            !PROVIDERS.includes(provider as PullRequestProvider) ||
            !isText(owner) ||
            !isText(repo) ||
            typeof number !== "number" ||
            !Number.isSafeInteger(number) ||
            number <= 0
        ) {
            return null
        }
        return {
            kind: "pullRequest",
            reference: { provider: provider as PullRequestProvider, owner, repo, number },
            path: value.path,
            head,
            base,
            sides,
        }
    }
    return null
}

/// Where the browser skin opens a file's zoom tab: the application's own
/// document, the window flag and the address beside it, as the desktop
/// window's page carries them.
export function imageWindowPath(address: string): string {
    return `/?imageWindow=1&at=${encodeURIComponent(address)}`
}

/// The tab's name, which makes a second Zoom on one file reuse its tab.
export function imageWindowName(address: string): string {
    return `specforge-image:${shortHash(address)}`
}

/// The window's title: the file's path.
export function imageWindowTitle(source: ImageWindowSource): string {
    return `${source.path} — zoom`
}
