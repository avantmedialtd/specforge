import React from "react"
import ReactDOM from "react-dom/client"
import App from "./App"
import { isTauri } from "./api"
import { isReaderRequest, ReaderRoot } from "./components/ReaderRoot"
import { readMirroredDocumentWidth } from "./docWidth"
import { usesMacTitlebarChrome } from "./platform"
import "./fonts.css"
import "./App.css"

// One bundle, two surfaces. A reader window loads this same document with
// `?reader=1`, which is read once here and nowhere else: the flag rides in the
// query rather than in the Address, so `encodeAddress`/`decodeAddress` stay
// pure and the same path names the same document whether it is opened as a
// reader or in the full application (`reader-window`: *Reader Presentation Is
// Not Part of the Address*).
const reader = isReaderRequest(window.location.search)

// Set body[data-platform="mac"] before React mounts so CSS that keys off
// it — sidebar transparency over vibrancy, traffic-light safe-area — is
// in effect from the first paint. Gated on the native window, not the
// user-agent: see the note on usesMacTitlebarChrome.
//
// A reader window is excluded: it carries a NATIVE titlebar rather than the
// main window's overlay one, so the traffic-light clearance that layout
// reserves would leave a band of empty space under a real titlebar.
if (usesMacTitlebarChrome(isTauri(), navigator.userAgent, reader)) {
    document.body.dataset.platform = "mac"
}
if (reader) {
    document.body.dataset.surface = "reader"
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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>{reader ? <ReaderRoot /> : <App />}</React.StrictMode>,
)
