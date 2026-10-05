import { OpenInWindow } from "./icons"

/// The visible way to open the current document in its own window.
///
/// Cmd/Ctrl-click on a row does the same thing, but a modifier chord is
/// invisible — and on a touch device there is no modifier key at all, which
/// would leave reader windows unreachable rather than merely undiscovered.
/// So this control follows the same contract the figure-maximize affordance
/// does: a real button (hence keyboard-operable), revealed on hover where
/// hover exists, rendered at rest where it does not, and given an enlarged
/// hit area on a coarse pointer — see the *Essential Controls Are
/// Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size
/// on Coarse Pointers* requirements in the `touch-input` capability.
///
/// A module of its own, so a surface that renders no document places this
/// control rather than a second one: the pull-request view's pop-out is the
/// same control (`pull-request-viewer`: *Pull-Request View*).
export function OpenReaderControl({ onClick }: { onClick: () => void }) {
    return (
        <button
            type="button"
            className="identity-open-reader"
            onClick={onClick}
            aria-label="Open in its own window"
            title="Open in its own window"
        >
            <OpenInWindow width={13} height={13} />
        </button>
    )
}
