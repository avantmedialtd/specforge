import React, { type ReactElement } from "react"
import ReactDOM from "react-dom/client"
import App from "./App"
import { isTauri } from "./api"
import { ImageWindowRoot } from "./components/ImageWindowRoot"
import { installHeadPolicies, PullRequestWindowRoot } from "./components/PullRequestWindowRoot"
import { ReaderRoot } from "./components/ReaderRoot"
import { readMirroredDocumentWidth } from "./docWidth"
import { usesMacTitlebarChrome } from "./platform"
import { windowKind, type WindowKind } from "./windowKind"
import "./fonts.css"
import "./App.css"

// One bundle, three surfaces. A reader window loads this same document with
// `?reader=1`, and a pull-request window with `?pullRequest=1`, and the flag is
// read once here, by `windowKind`, and nowhere else: it rides in the query
// rather than in the Address, so `encodeAddress`/`decodeAddress` stay pure and
// the same path names the same thing whichever root shows it (`reader-window`:
// *Reader Presentation Is Not Part of the Address*; `view-routing`:
// *Addressable Viewing State*).
const kind = windowKind(window.location.search)

// Set body[data-platform="mac"] before React mounts so CSS that keys off
// it — sidebar transparency over vibrancy, traffic-light safe-area — is
// in effect from the first paint. Gated on the native window, not the
// user-agent: see the note on usesMacTitlebarChrome.
//
// The detached windows are excluded: a reader and a pull-request window carry
// a NATIVE titlebar rather than the main window's overlay one, so the
// traffic-light clearance that layout reserves would leave a band of empty
// space under a real titlebar.
if (usesMacTitlebarChrome(isTauri(), navigator.userAgent, kind)) {
    document.body.dataset.platform = "mac"
}
// Each detached window's own surface rule, which gives the whole window to its
// one view.
if (kind === "reader") {
    document.body.dataset.surface = "reader"
} else if (kind === "pullRequest") {
    document.body.dataset.surface = "pull-request"
} else if (kind === "imageWindow") {
    document.body.dataset.surface = "image"
}

// Set body[data-doc-width] before React mounts, for the same reason as the two
// stamps above: the reading width decides the content column's geometry, so a
// surface that painted at the default and then adopted the stored rung would
// reflow the whole document on every cold start — the most visible flash this
// application could produce.
//
// Read from the synchronous `localStorage` mirror rather than from the settings
// store, which is behind an async IPC call and cannot answer before the first
// frame. The mirror is a hint, not the source of truth: `useDocumentWidth`
// fetches the authoritative value on mount and re-stamps if they disagree,
// which is what corrects a width changed by another instance since this window
// last ran. An absent or unreadable mirror yields the default rung, so this
// cannot throw and cannot stamp anything but a real rung.
document.body.dataset.docWidth = readMirroredDocumentWidth()

/// The root each window kind renders. A root renders only its own kind of
/// address: a reader given a pull request's address reads "Document not
/// found", and a pull-request window given any other reads "Pull request not
/// found".
function rootFor(kind: WindowKind): ReactElement {
    switch (kind) {
        case "application":
            return <App />
        case "reader":
            return <ReaderRoot />
        case "pullRequest":
            return <PullRequestWindowRoot />
        case "imageWindow":
            return <ImageWindowRoot />
    }
}

// The window kind's `head` policies, in place before its root renders, so
// before any fetch they govern starts: DNS prefetching off for every kind, and
// the content-security policy for the pull-request window's alone, never the
// application's or a reader's (`pull-request-viewer`: *Pull-Request Window
// Permissions*, *Pull-Request Content Is Untrusted*). Here, in the one path
// every kind takes to its render, and never from a root module, which this one
// imports whatever it renders, nor from an effect, which runs only after the
// children have mounted.
installHeadPolicies(kind, document)

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>{rootFor(kind)}</React.StrictMode>,
)
