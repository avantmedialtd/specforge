import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"

const host = process.env.TAURI_DEV_HOST

// Web-UI dev: proxy the command endpoint and SSE event stream to a running
// `specforge-serve` so `bun run dev` (Vite + HMR on 1420) drives the real
// backend. The standalone server's port is configurable; override the proxy
// target with `SPECFORGE_WEB_PORT` to match. The Tauri desktop build ignores
// this — it talks to the backend in-process, not over HTTP.
const webServerPort = process.env.SPECFORGE_WEB_PORT ?? "4317"

export default defineConfig({
    plugins: [react()],
    clearScreen: false,
    build: {
        // Pinned to what Vite 5 defaulted to (its ESBUILD_MODULES_TARGET).
        // Vite 7 changed the default to `baseline-widely-available` and Vite 8
        // advanced that baseline to chrome111/edge111/firefox114/safari16.4/
        // ios16.4 — which would quietly raise this bundle's syntax floor past
        // the platforms the app itself claims to support: tauri.conf.json
        // declares `minimumSystemVersion: "11.0"`, and macOS 11 ships the
        // WebKit that pairs with Safari 14, while index.html advertises the web
        // UI as an iOS home-screen install. Moving that floor is a product
        // decision about which machines still run SpecForge, so it should be
        // its own change rather than a side effect of a build-tool upgrade.
        //
        // Note this also pins the CSS pipeline: Vite resolves
        // `cssTarget ?? target`, so Lightning CSS downlevels App.css to the
        // same floor.
        target: ["es2020", "edge88", "firefox78", "chrome87", "safari14"],
    },
    server: {
        port: 1420,
        strictPort: true,
        host: host || false,
        proxy: {
            "/api": {
                target: `http://127.0.0.1:${webServerPort}`,
                changeOrigin: false,
            },
        },
        hmr: host
            ? {
                  protocol: "ws",
                  host,
                  port: 1421,
              }
            : undefined,
        watch: {
            ignored: ["**/crates/**", "**/target/**"],
        },
    },
})
