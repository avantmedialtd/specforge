// Opening a pull request the way a document opens (`pull-request-viewer`:
// *Opening a Pull Request Like a Document*, *Pull-Request Window*,
// *Pull-Request Window Title*; design D3 and D4).
//
// A click shows the pull request in the center pane, at its address, and the
// platform's new-window gesture opens its own window: a desktop window, or a
// browser tab in the browser skin. What a click asks for and what it then does,
// which address a row opens, where the window lives and what it is called are
// decided here, as pure functions, for the reason `pullRequestLinks.ts` gives:
// JSX is not exercised by `bun test` and a frontend-only diff skips the
// mutation gate, so these tests are the only coverage the decisions get.

import { isNewWindowModifier } from "./platform"
import { referenceOf, type PullRequestAddress } from "./routing/address"
import { encodeAddress } from "./routing/codec"
import { shortHash } from "./routing/slug"
import type { PullRequestProvider, PullRequestReference, PullRequestSummary } from "./types"

/// What a click on a pull-request row or chip asks for:
///
/// - `navigate` — show the pull request in the center pane, at its address;
/// - `window` — open its own window, or focus the one already open, and
///   change nothing else;
/// - `nothing` — the macOS secondary click (a Ctrl-click), which opens
///   nothing;
/// - `default` — any other modified click, left to the element itself, so a
///   browser-skin link's Shift-click still opens its address in a new browser
///   window.
///
/// A handler calls `preventDefault` for every outcome but `default`. Keyboard
/// activation needs no decision of its own: Enter and Space on a button, and
/// Enter on a link, arrive as a click, and a link's Space is turned into one by
/// `isActivationSpace`, so a key press decides exactly as the click it becomes.
export type PullRequestGesture = "navigate" | "window" | "nothing" | "default"

/// The modifier keys a click carries. A React `MouseEvent` is one.
export interface ClickModifiers {
    metaKey: boolean
    ctrlKey: boolean
    shiftKey: boolean
    altKey: boolean
}

export function pullRequestGesture(
    click: ClickModifiers,
    userAgent: string = navigator.userAgent,
): PullRequestGesture {
    // Before the new-window modifier: on macOS every click with Ctrl held is
    // taken as the secondary click, Cmd-Ctrl included, so the gesture never
    // opens a window beneath a context menu.
    if (/Mac/i.test(userAgent) && click.ctrlKey) return "nothing"
    if (isNewWindowModifier(click, userAgent)) return "window"
    if (click.metaKey || click.ctrlKey || click.shiftKey || click.altKey) return "default"
    return "navigate"
}

/// Whether a key press activates a browser-skin row or chip beyond the link's
/// own Enter: the Space key (`" "`; `"Spacebar"` in older engines). A link
/// activates on Enter only, so its handler turns Space into the click a button
/// would have had.
export function isActivationSpace(key: string): boolean {
    return key === " " || key === "Spacebar"
}

/// Where a click on a row or a chip takes its pull request.
export interface PullRequestOpeners {
    /// Show it in the center pane, at its address: App's `openPullRequest`,
    /// which adds a history entry.
    navigate: (address: PullRequestAddress) => void
    /// Open its own window, or focus the one already open: `openPullRequestWindow`,
    /// with the address's path and the window's title.
    openWindow: (addressPath: string, title: string) => void
}

/// Act on a click on a pull-request row or chip, as `pullRequestGesture`
/// decides it, and prevent its default for every outcome but `default`. The
/// window is opened before this returns, so it opens inside the click, where no
/// popup blocker intervenes, and nothing else is touched: not the selection,
/// the center pane or the history. `title` is the pull request's title as its
/// row knows it, for the window's title.
export function handlePullRequestClick(
    click: ClickModifiers & { preventDefault(): void },
    address: PullRequestAddress,
    title: string | null,
    openers: PullRequestOpeners,
    userAgent?: string,
): void {
    const gesture = pullRequestGesture(click, userAgent)
    if (gesture === "default") return
    click.preventDefault()
    if (gesture === "navigate") {
        openers.navigate(address)
    } else if (gesture === "window") {
        openers.openWindow(encodeAddress(address), pullRequestTitle(referenceOf(address), title))
    }
}

/// The address a row or chip opens, with the owner and repository spelt as the
/// row spells them, so every launch of one pull request encodes one address
/// and opens one window, however a link spelt it (`view-routing`:
/// *Pull-Request Addresses*). It is also the canonical spelling
/// `resolvePullRequestAddress` returns for a listed pull request.
///
/// `null` for a row whose web URL is empty, such as a GitHub row whose link
/// was foreign, which opens nothing; and for a row no path could name — a
/// repository that is not `owner/repo`, or a number that is not a positive
/// whole number.
export function pullRequestAddressFor(
    provider: PullRequestProvider,
    row: Pick<PullRequestSummary, "id" | "repoFullName" | "url">,
): PullRequestAddress | null {
    if (row.url === "") return null
    const slash = row.repoFullName.indexOf("/")
    if (slash <= 0 || slash === row.repoFullName.length - 1) return null
    if (!Number.isSafeInteger(row.id) || row.id < 1) return null
    return {
        kind: "pullRequest",
        provider,
        owner: row.repoFullName.slice(0, slash),
        repo: row.repoFullName.slice(slash + 1),
        number: row.id,
    }
}

/// Where the browser skin opens a pull request's own tab: its address's path,
/// with the window flag outside that path, so the center pane and the window
/// share one path (`view-routing`: *Addressable Viewing State*).
/// `addressPath` is an `encodeAddress` result.
export function pullRequestWindowPath(addressPath: string): string {
    return `${addressPath}?pullRequest=1`
}

/// The name of that tab. The name is what makes a second gesture on the same
/// pull request reuse and focus the tab instead of opening another, and the
/// hash of the path keeps two pull requests' tabs apart, as it keeps readers'.
export function pullRequestWindowName(addressPath: string): string {
    return `specforge-pull-request:${shortHash(addressPath)}`
}

/// The longest title a pull-request window carries, in code points. It is the
/// cap `openspec-app`'s window-title sanitiser applies to the desktop's
/// titlebar, so the title the page sets is the title that titlebar shows.
export const PULL_REQUEST_TITLE_CAP = 200

/// Every default-ignorable character — the bidirectional controls, the
/// zero-width characters, tags, variation selectors — and every control
/// character.
const HIDDEN_OR_CONTROL = /[\p{Default_Ignorable_Code_Point}\p{Cc}]/gu

/// A pull-request window's title: `#42 Add rate limits — acme/api`, or
/// `#42 — acme/api` while its title is not known, where a BitBucket pull
/// request's owner is its workspace. Every default-ignorable and control
/// character is removed, title and names alike, since a stranger's right-to-
/// left override would otherwise reorder the titlebar, and the result is cut
/// to `PULL_REQUEST_TITLE_CAP` code points: by code point, so the cut never
/// falls inside a surrogate pair and leaves half a character to draw.
export function pullRequestTitle(reference: PullRequestReference, title?: string | null): string {
    const name = withoutHidden(title ?? "").trim()
    const repository = `${withoutHidden(reference.owner)}/${withoutHidden(reference.repo)}`
    const full =
        name === ""
            ? `#${reference.number} — ${repository}`
            : `#${reference.number} ${name} — ${repository}`
    return [...full].slice(0, PULL_REQUEST_TITLE_CAP).join("")
}

function withoutHidden(text: string): string {
    return text.replace(HIDDEN_OR_CONTROL, "")
}
