import React from "react"
import ReactDOM from "react-dom/client"
import App from "./App"
import "./fonts.css"
import "./App.css"

// Set body[data-platform="mac"] before React mounts so CSS that keys off
// it — sidebar transparency over vibrancy, traffic-light safe-area — is
// in effect from the first paint.
if (/Mac/i.test(navigator.userAgent)) {
    document.body.dataset.platform = "mac"
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
        <App />
    </React.StrictMode>,
)
