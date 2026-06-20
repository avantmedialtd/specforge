# Optional Local Web UI

## Why

SpecForge already renders the same OpenSpec state through two skins — the Tauri desktop app and the `specforge-tui` terminal frontend — over one shared brain, `openspec-app::AppService`. The refactor that pulled all stateful orchestration (settings, presentation, activity log, watcher lifecycle, dashboard assembly) out of the Tauri commands and down into `AppService` did the hard, frontend-agnostic part already. A web skin is the natural third consumer of that brain.

The motivation is reach without a rewrite. Some users prefer a browser tab to a native window; some run SpecForge on a box they mostly look at through a browser; some want to drop the dashboard onto a second monitor without a second native app. Today none of that is possible — the only way to *see* the state is to launch the desktop binary or the TUI.

Crucially, the cost of saying yes is small *because of how the codebase is already shaped*. The React frontend funnels essentially all of its backend coupling through one file, `src/api.ts` (~30 commands via Tauri `invoke`, ~12 events via `listen`). Everything else imports from `api.ts` or imports types only. Swap that one file's transport from Tauri IPC to HTTP + SSE and the entire React app runs in a browser nearly unchanged. The backend already cooperates too: `register_workspace` takes a plain path string, so the only genuinely native frontend concern — the folder picker — is a frontend swap, not a backend one.

This change scopes the web UI to its cheapest, local-first-preserving form: **local self-served**. A server bound to localhost reflects *this machine's* registered workspaces — the browser is an alternative skin to the desktop app, not a hosted multi-user product. Remote single-user access (auth, TLS, network binding) and any hosted/team model (multi-tenant registry, leaderboard across real people) are explicitly out of scope; they are different products that would break the local-first model the registry, watcher, and git mining all assume.

## What Changes

- **New `specforge-web` crate** — a library that turns an `AppService` into an `axum` application (`router(svc) -> Router`, `serve(svc, addr)`), plus a thin `specforge serve` binary entry point. It depends on `openspec-core` and `openspec-app` only — no Tauri.
- **One `POST /api/invoke` endpoint** mirroring Tauri's `invoke(command, args)` shape: `{ command, args }` in, JSON result out. A dispatch table maps the ~30 command names onto `AppService` / `SettingsStore` / `WatcherManager` methods. New commands need no new routes.
- **One `GET /api/events` SSE stream** bridging the watcher's existing `CacheEvent` broadcast to `text/event-stream`, reproducing the exact event *names* and *payload shapes* the frontend already listens for, so the unchanged React handlers fire.
- **Two entry points around one server core.** The standalone binary bootstraps its own `AppService` from the shared config dir (exactly as `specforge-tui` does today); the desktop app, behind a Settings toggle, hands `serve()` a clone of the `AppService` it already holds — so the embedded case shares one watcher and the browser becomes a live mirror of desktop state, with zero state contention.
- **Frontend transport abstraction.** `src/api.ts` detects its host (`window.__TAURI__` present → `invoke`/`listen`; absent → `fetch`/`EventSource`) and dispatches accordingly. Same bundle, two hosts; the same `dist/` is embedded in the Tauri app and served by the web crate.
- **Web-flavoured affordances** branching on the same host flag: a path-input (or server-side directory browser) replacing the native folder dialog; window-control stubs replacing `getCurrentWindow()`; and the Settings view hiding desktop-only controls (launch-on-login, OS notifications, tray) — reusing the existing "hide the control when its command returns null" pattern already used for the WSL poll interval.
- **Extraction refactor (the bonus payoff).** Two pieces of "skin-specific-but-not-really" logic move down into `openspec-app` so all three skins consume one copy: the `read_artifact` path-traversal guard (today in `specforge/commands.rs`) and the `CacheEvent → (name, payload)` mapping plus its payload structs (today in `specforge/events.rs`). The Tauri forwarder and the SSE bridge then become two sinks over one shared mapping.
- **Localhost trust boundary.** The server binds `127.0.0.1` only and validates the `Origin`/`Host` header (optionally gated by a token embedded in the URL the app opens), so an unrelated web page in the user's browser cannot drive `register_workspace` / `read_artifact` against local files.

## Capabilities

### New Capabilities

- `web-ui`: a local, optional, self-served browser skin for the same OpenSpec state the desktop and terminal frontends render. Covers the localhost HTTP+SSE server, the `invoke`-mirroring command transport, the `CacheEvent`→SSE event bridge, the two entry points (embedded toggle + standalone `serve`), the localhost trust boundary, and the web-flavoured affordances (path-based workspace registration, hidden desktop-only settings). Parallels the existing `terminal-ui` capability.

### Modified Capabilities

- `workspace-registry`: clarifies that workspace registration accepts a path string from any frontend — the native OS folder dialog is one frontend's way of producing that string, not a registry requirement. The web frontend supplies the path by text input or a server-side directory browser.
- `spec-browser`: the `read_artifact` path-traversal guard becomes a shared `openspec-app` concern rather than a Tauri-command concern, so every frontend (including the web server) enforces the identical "resolved path must stay under `openspec/changes/`" invariant.

## Impact

- **Code (new crate `specforge-web`)**: `lib.rs` (`router`/`serve`), `dispatch.rs` (the `/api/invoke` command table, ~30 mechanical arms), `sse.rs` (`CacheEvent` broadcast → SSE), `main.rs` (the `specforge serve` binary). New runtime deps: `axum`, `tower-http` (static-file + CORS/host layers), `tokio` (already in the workspace), and an asset-embedding crate (`rust-embed`) for production static serving.
- **Code (`openspec-app`)**: gains the `read_artifact` guard and the `CacheEvent → (name, payload)` mapping + payload structs (moved down from the Tauri crate), exported for both the shell's forwarder and the web crate's SSE bridge.
- **Code (`specforge` shell)**: `events.rs` reduces to a thin sink calling the shared mapping; `commands.rs::read_artifact` delegates to the shared guard; `lib.rs` and `settings.rs` gain an opt-in "serve web UI on :PORT" toggle that calls `specforge_web::serve(svc.clone(), addr)`.
- **Code (frontend)**: `src/api.ts` grows a host-detection branch and an `EventSource`-based event layer; `SettingsView` gains a web folder-input path and hides desktop-only controls in web mode; `App.tsx` stubs `getCurrentWindow()` usage when not under Tauri. Components, hooks, and `src/types.ts` are otherwise unchanged.
- **Build / packaging**: the existing `dist/` bundle is embedded into `specforge-web` for production; in dev the server proxies to or serves the Vite build so HMR still works. A new `specforge serve` binary is added to the workspace members and (optionally) to release packaging.
- **Persistence / config**: none new beyond an optional `web` settings block (enabled flag, port, bind address) in the existing settings store. Both entry points resolve the same `app_config_dir()`, so a standalone `serve` running *alongside* the desktop app reintroduces the known two-writer `activity.json` contention; the embedded toggle avoids it by sharing one `AppService`.
- **Out of scope**: remote/network exposure (binding beyond `127.0.0.1`), authentication, TLS; any hosted or multi-user/team deployment; a leaderboard across real distinct developers (the gamification layer resolves to a single canonical developer today and that is unchanged); creating/editing OpenSpec artifacts from the web UI (read-only parity with the desktop app, as today); mobile-specific responsive redesign of the existing components.
