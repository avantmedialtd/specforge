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
