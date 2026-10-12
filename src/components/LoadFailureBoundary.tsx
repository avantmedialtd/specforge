import { Component, type ReactNode } from "react"
import type { Address, Unresolvable } from "../routing/address"
import { encodeAddress } from "../routing/codec"
import { EmptyState } from "./EmptyState"

interface LoadFailureBoundaryProps {
    children: ReactNode
    /// A value naming what is shown (`surfaceResetKey`): when it changes, a
    /// failure shown for the previous one is forgotten and the children are
    /// rendered again. That helps only when the reader moves to another
    /// surface or another thing on it: `React.lazy` keeps its rejection, and
    /// the browser keeps a failed module fetch, so the same surface fails the
    /// same way until the page reloads. Reload is the real recovery.
    resetKey?: string
    /// What Reload does; the page reloads when none is given. In the desktop
    /// main window, whose history lives in memory, a reload lands on the
    /// Dashboard, and the document watches the window holds are released only
    /// when the window is destroyed. Both are acceptable on a failure path.
    onReload?: () => void
}

interface LoadFailureBoundaryState {
    /// What failed, or null while nothing has.
    failure: "load" | "render" | null
    resetKey: string | undefined
}

/// Whether `error` is a chunk that could not be loaded, as each engine words
/// it: Chromium's and WebKit's dynamic-import failures, Firefox's, and Vite's
/// stylesheet preload.
export function isChunkLoadError(error: unknown): boolean {
    const message = error instanceof Error ? error.message : String(error)
    return /dynamically imported module|Importing a module script failed|Unable to preload CSS/i.test(
        message,
    )
}

/// The reset key of what the detail pane shows: its address and, beside it,
/// the commit the rail selected, which is not part of the address.
export function surfaceResetKey(address: Address | Unresolvable, commitId: string | null): string {
    const path = address.kind === "unresolvable" ? "" : encodeAddress(address)
    return `${path}\u0000${commitId ?? ""}`
}

/// Where a lazily loaded surface's failure stops (`spec-browser`: *Surfaces
/// and Rendering Engines Load on Demand*): a chunk that cannot be read
/// rejects its `React.lazy`, and without a boundary React unmounts the whole
/// root. This says so in place, with a way to reload, and leaves the rest of
/// the window as it was. It catches any render error beneath it, and words
/// one that is not a failed load neutrally.
export class LoadFailureBoundary extends Component<
    LoadFailureBoundaryProps,
    LoadFailureBoundaryState
> {
    state: LoadFailureBoundaryState = { failure: null, resetKey: this.props.resetKey }

    static getDerivedStateFromError(error: unknown): Partial<LoadFailureBoundaryState> {
        return { failure: isChunkLoadError(error) ? "load" : "render" }
    }

    static getDerivedStateFromProps(
        props: LoadFailureBoundaryProps,
        state: LoadFailureBoundaryState,
    ): Partial<LoadFailureBoundaryState> | null {
        return props.resetKey === state.resetKey
            ? null
            : { failure: null, resetKey: props.resetKey }
    }

    componentDidCatch(error: unknown): void {
        console.warn("failed to show a view:", error)
    }

    render(): ReactNode {
        if (this.state.failure === null) return this.props.children
        return <LoadFailureNotice failure={this.state.failure} onReload={this.props.onReload} />
    }
}

/// What a surface that could not be shown says.
export function LoadFailureNotice({
    failure,
    onReload,
}: {
    failure: "load" | "render"
    onReload?: () => void
}) {
    return (
        <EmptyState
            title={
                failure === "load"
                    ? "Couldn’t load this view"
                    : "Something went wrong showing this view"
            }
            body={
                <>
                    {failure === "load" && <p>Part of SpecForge could not be loaded.</p>}
                    <button
                        type="button"
                        className="archive-back"
                        onClick={onReload ?? (() => window.location.reload())}
                    >
                        Reload
                    </button>
                </>
            }
        />
    )
}
