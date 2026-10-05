import type { MouseEvent, ReactNode } from "react"
import { isWeb, openPullRequestWindow } from "../api"
import { handlePullRequestClick, isActivationSpace } from "../pullRequestOpen"
import type { PullRequestAddress } from "../routing/address"
import { encodeAddress } from "../routing/codec"

/// One pull request as a control: a panel row, or a header chip. Both open it
/// the way a document opens (`pull-request-viewer`: *Opening a Pull Request
/// Like a Document*): a click, Enter or Space shows it in the center pane
/// through `onOpen`, and the platform's new-window gesture opens its own
/// window, inside the click and with nothing else touched. What each click
/// does is `handlePullRequestClick`'s decision.
///
/// A button on the desktop, where a link would let the webview's own "Open
/// Link" reload the main window at the pull request's path, and the in-memory
/// history would be lost with the view. In the browser skin a link to the pull
/// request's SpecForge address, never the provider's page, so middle-click,
/// "Open Link in New Tab" and Copy Link all lead to SpecForge at that address.
export function PullRequestControl({
    address,
    title,
    onOpen,
    className,
    label,
    children,
}: {
    address: PullRequestAddress
    /// The pull request's title as its row knows it, for its window's title.
    title: string
    onOpen: (address: PullRequestAddress) => void
    className: string
    /// The tooltip and accessible name, for a control whose content does not
    /// say enough on its own. Never the provider's URL.
    label?: string
    children: ReactNode
}) {
    const onClick = (event: MouseEvent<HTMLElement>) =>
        handlePullRequestClick(event, address, title, {
            navigate: onOpen,
            openWindow: openPullRequestWindow,
        })
    if (isWeb()) {
        return (
            <a
                className={className}
                href={encodeAddress(address)}
                title={label}
                aria-label={label}
                onClick={onClick}
                // A link activates on Enter only. A synthetic click turns
                // Space into one, which `onClick` then decides as any other.
                onKeyDown={(event) => {
                    if (isActivationSpace(event.key)) {
                        event.preventDefault()
                        event.currentTarget.click()
                    }
                }}
            >
                {children}
            </a>
        )
    }
    return (
        <button
            type="button"
            className={className}
            title={label}
            aria-label={label}
            onClick={onClick}
        >
            {children}
        </button>
    )
}
