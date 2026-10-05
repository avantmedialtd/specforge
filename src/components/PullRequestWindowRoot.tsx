import { useMemo, useState } from "react"
import { setPullRequestWindowSize } from "../api"
import { useDetachedWindow } from "../hooks/useDetachedWindow"
import { useDocumentWidth } from "../hooks/useDocumentWidth"
import { usePullRequestLinks } from "../hooks/usePullRequestLinks"
import { usePullRequestProviderFlags } from "../hooks/usePullRequestProviderFlags"
import { usePullRequestSnapshot } from "../hooks/usePullRequestSnapshot"
import { useWorkspaces } from "../hooks/useWorkspaces"
import { pullRequestTitle } from "../pullRequestOpen"
import { referenceOf } from "../routing/address"
import { decodeAddress } from "../routing/codec"
import { resolvePullRequestAddress } from "../routing/resolve"
import { EmptyState } from "./EmptyState"
import { PullRequestAtAddress } from "./PullRequestView"
import { readerAddressPath } from "./ReaderRoot"

// The pull-request window's `head` policies are installed by `main.tsx`, in its
// branch for this root and before it renders, never from here: `main.tsx`
// imports this module whichever root it renders, so code at this module's
// scope would run in every window, and an effect runs only after the children
// have mounted (design D10).
export { installHeadPolicies } from "../windowKind"

/// The pull-request window: one pull request apart from the main window, and
/// nothing else of it — no workspace tree, no commit rail, no pull-request
/// panel (`pull-request-viewer`: *Pull-Request Window*). A native window on the
/// desktop and a browser tab in the browser skin.
///
/// It takes its address as a reader does, from `at` on the desktop and from
/// the path in the browser skin, the `pullRequest` flag riding outside it, and
/// reads "Pull request not found" for any other kind of address. It resolves
/// that address itself, against the provider flags and snapshots it reads and
/// keeps current, so it shows the center pane's outcomes, names Settings ›
/// Integrations as text, and replaces the pull request with the provider-off
/// notice when its provider is switched off. It renders the view with no
/// pop-out control, since it is already detached, and its linked change
/// navigates nothing. Of the native window it uses only invoke, listen and
/// close, which its narrow capability grants.
export function PullRequestWindowRoot() {
    const addressPath = useMemo(
        () => readerAddressPath(window.location.search, window.location.pathname),
        [],
    )
    const address = useMemo(() => {
        const decoded = decodeAddress(addressPath)
        return decoded.kind === "pullRequest" ? decoded : null
    }, [addressPath])

    const flags = usePullRequestProviderFlags()
    const github = usePullRequestSnapshot("github")
    const bitbucket = usePullRequestSnapshot("bitbucket")
    // The views and links only name the linked change, which the window shows
    // as passive text.
    const { views } = useWorkspaces()
    const links = usePullRequestLinks(views)
    // Its own reconciliation and listener for the reading width, as a reader
    // window has, since this root never passes through `App`.
    useDocumentWidth()

    const resolution = useMemo(
        () => (address ? resolvePullRequestAddress(address, flags, { github, bitbucket }) : null),
        [address, flags, github, bitbucket],
    )

    // The title the view shows, which a read brings, and before any the row's,
    // under the row's spelling while it is listed (`pull-request-viewer`:
    // *Pull-Request Window Title*).
    const [shownTitle, setShownTitle] = useState<string | null>(null)
    const listed = resolution?.status === "listed" ? resolution : null
    const title = address
        ? pullRequestTitle(
              referenceOf(listed ? listed.address : address),
              shownTitle ?? listed?.row.title ?? null,
          )
        : "SpecForge"
    useDetachedWindow(title, setPullRequestWindowSize)

    if (!address || !resolution) {
        return (
            <EmptyState
                title="Pull request not found"
                body="This window's address doesn't name a pull request."
            />
        )
    }

    return (
        <PullRequestAtAddress
            address={address}
            resolution={resolution}
            views={views}
            links={links}
            onTitle={setShownTitle}
        />
    )
}
