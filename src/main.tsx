import React from "react"
import ReactDOM from "react-dom/client"
import App from "./App"
import { isTauri } from "./api"
import { usesMacTitlebarChrome } from "./platform"
import "./fonts.css"
import "./App.css"

// Set body[data-platform="mac"] before React mounts so CSS that keys off
// it — sidebar transparency over vibrancy, traffic-light safe-area — is
// in effect from the first paint. Gated on the native window, not the
// user-agent: see the note on usesMacTitlebarChrome.
if (usesMacTitlebarChrome(isTauri(), navigator.userAgent)) {
    document.body.dataset.platform = "mac"
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
        <App />
    </React.StrictMode>,
)
