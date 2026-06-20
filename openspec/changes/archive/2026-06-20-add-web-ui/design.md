# Design

## Context

SpecForge is a three-layer split already:

```
openspec-core      pure primitives — registry, parser, cache, watcher, git mining,
                   dashboard math. No Tauri, no terminal. Fully cargo-testable.
      │
openspec-app       AppService: the stateful brain — settings, presentation, activity
   (AppService)    log, watcher lifecycle, dashboard assembly, quota. Emits CacheEvents
      │            on a tokio broadcast channel. Cheaply cloneable (every field an Arc).
      ├─────────────────────────────┐
specforge (Tauri)            specforge-tui (ratatui)
#[command] wrappers          in-process AppService calls
+ React frontend
```

`AppService` is the seam this change builds on. Both existing frontends are thin: the TUI calls `AppService` methods in-process; the Tauri shell wraps those same methods in `#[tauri::command]` and bridges `CacheEvent`s to named Tauri events. A web UI is a third skin over the identical brain — the only genuinely new thing is the *transport* between a browser and `AppService`.

Two facts discovered in the existing code make this cheap rather than a rewrite:

1. **The frontend has one backend seam.** `src/api.ts` is the sole runtime touchpoint for Tauri `invoke`/`listen`. Of the few other files importing `@tauri-apps`, all but two import *types only* (`import type { UnlistenFn }`, zero runtime coupling). The two real leaks are `getCurrentWindow()` in `App.tsx` and the native folder dialog in `SettingsView`.
2. **The backend is already path-string oriented.** `register_workspace(path: String)` takes a plain string; the native dialog that produces it lives entirely in the frontend. So "register a workspace" needs no backend change for the web — only a different way to produce the string.

## Goals / Non-Goals

**Goals:**

- Add a browser skin for the same OpenSpec state, reusing the existing React app with the smallest possible diff — ideally a transport swap in one file plus a handful of host-conditional branches.
- Keep the new backend surface to a transport adapter over `AppService`, introducing no new state, no new business logic, and no duplication of the dashboard / aggregation / settings logic that already lives in the core and the app layer.
- Design the server as a library with two entry points (embedded-in-desktop, standalone binary) so the v1 packaging decision is a thin caller choice, not an architectural fork.
- Preserve the local-first model: the server reflects *this machine's* registered workspaces, bound to localhost.
- Use the web crate as the forcing function to finish the `openspec-app` extraction — move the last skin-specific-but-shared logic (the `read_artifact` guard, the event mapping) down so all three skins consume one copy.

**Non-Goals:**

- Network exposure, authentication, TLS, or any remote-access story. Binding stays `127.0.0.1`.
- Any hosted, multi-tenant, or team deployment. The registry, watcher, and git mining all assume local paths owned by one developer; multi-user is a different product.
- A leaderboard / gamification view across distinct real developers. The gamified layer resolves to a single canonical developer today; this change does not touch that.
- Write access from the browser (toggling tasks, editing artifacts). The web skin has the same read-only parity the desktop app has today.
- A mobile-first responsive redesign. The existing components render in a browser; reflowing them for small screens is a later, separate concern.
- Sharing the dispatch layer between Tauri's `#[command]` macro and the web crate via a unified command enum. Tempting for DRY, but Tauri wants individual functions with `State` extractors; the duplication is plumbing (arg-deserialize + method-call), not logic, and forcing a shared enum costs more than it saves for v1.

## Decisions

### One `/api/invoke` endpoint that mirrors Tauri's `invoke(command, args)`

The web command transport is a single `POST /api/invoke` taking `{ command: string, args: object }` and returning the JSON result (or an error envelope). This mirrors the Tauri `invoke(command, args)` contract exactly, which makes the `src/api.ts` swap a change to *one wrapper function* rather than ~30 per-command fetch calls, and means a new backend command needs a new dispatch arm but no new HTTP route or frontend route.

The dispatch table is a `match command { ... }` with one arm per command: deserialize `args` into the command's parameter shape, call the corresponding `AppService` / `SettingsStore` / `WatcherManager` method, serialize the result. This is the bulk of the new "real" code, but it is mechanical and parallels the existing `commands.rs` handlers (which already do nothing but extract state and call the same methods).

**Alternatives considered:** REST-style `POST /api/<command>` routes — rejected because it multiplies routes and `api.ts` call sites for zero benefit over the `invoke` mirror, and diverges the web call shape from the Tauri one the frontend already speaks. JSON-RPC framing — rejected as unnecessary ceremony; we don't need batching, notifications, or id-correlation for a request/response command surface.

### SSE, not WebSocket, for the event stream

Every event in the system is **server → client only** (`cache-updated`, `change-added`, `graph-changed`, `quota-updated`, …); commands are a separate request/response channel. That is precisely the shape of Server-Sent Events. `GET /api/events` subscribes to the watcher's existing `CacheEvent` broadcast and emits each as an SSE frame whose `event:` name and `data:` payload match what the frontend already listens for. The browser side uses the native `EventSource`, which reconnects automatically — a property we'd otherwise hand-roll over a socket.

**Alternatives considered:** WebSocket — rejected because it earns its keep only when the client also *sends* over the same socket; here commands go over HTTP, so a bidirectional socket adds reconnect/heartbeat/framing complexity for no gain. Long-polling — rejected as strictly worse than SSE for a push stream.

### Event mapping moves down into `openspec-app`; the SSE bridge and the Tauri forwarder are two sinks over it

Today `specforge/events.rs` owns both the payload structs (`CacheUpdatedPayload`, `ChangeAddedPayload`, …) and the `CacheEvent → (name, payload)` mapping, then `app.emit(name, payload)`s each. The SSE bridge must reproduce that mapping *byte-for-byte* or the unchanged React handlers won't fire. Rather than duplicate it, the mapping and the payload structs move into `openspec-app` as a pure function — roughly `fn event_envelope(e: &CacheEvent) -> (&'static str, serde_json::Value)`. The Tauri forwarder becomes `let (name, json) = event_envelope(&e); app.emit(name, json)`; the SSE bridge becomes `let (name, json) = event_envelope(&e); yield sse_frame(name, json)`. One mapping, two sinks, guaranteed-identical wire shapes.

### The `read_artifact` path-traversal guard moves down into `openspec-app`

The guard that canonicalizes the resolved artifact path and rejects anything outside the workspace's `openspec/changes/` subtree currently lives inside the Tauri command. The web server must enforce the same invariant. Moving it into `openspec-app` (a function the command and the dispatch arm both call) means the security-relevant check has exactly one implementation that `cargo test` covers directly, instead of a copy in each skin that can drift.

### One server core, two entry points

`specforge-web` exposes `serve(svc: AppService, addr: SocketAddr)` (and `router(svc) -> Router` for testing). Because `AppService` is `Clone` and shares state via `Arc`, the two entry points differ only in who builds the service:

```
EMBEDDED (desktop toggle)            STANDALONE (`specforge serve`)
─────────────────────────            ──────────────────────────────
desktop lib.rs already has an        bin/main.rs bootstraps its own:
AppService in Tauri State;             let svc = AppService::bootstrap(config_dir);
on the "serve web on :PORT"            svc.populate(); // same as specforge-tui
toggle it calls:                       specforge_web::serve(svc, addr).await
  specforge_web::serve(
    svc.clone(), addr)               browser is the only skin; no tray, no dock
ONE watcher, ONE AppService;
browser = live mirror of desktop
```

The embedded path shares one `AppService` and therefore one watcher and one writer of `activity.json` — no contention, and the browser observes exactly the desktop's live state. The standalone path is a true headless server for a GUI-less box. The two are not mutually exclusive long term; v1 can ship either caller first because both are ~10 lines around the same `serve()`.

**Known trade-off:** running the standalone `serve` binary *and* the desktop app simultaneously means two `AppService` instances against the same `app_config_dir()`, reintroducing the two-writer `activity.json` / window-state contention already documented for two desktop instances. The embedded toggle is the contention-free way to have both skins at once; the standalone binary is for "browser only."

### One React bundle, host-detected at runtime

`src/api.ts` becomes a thin dispatcher keyed on `window.__TAURI__`: present → the existing `invoke`/`listen` implementation; absent → a `fetch('/api/invoke', …)` + `new EventSource('/api/events')` implementation. The same built `dist/` is embedded in the Tauri app (as today) and served by `specforge-web` (via `rust-embed` in production, or proxied to Vite in dev so HMR survives). No build matrix, no compile-time flags — the window-control stubs and the folder-picker branch key on the same runtime flag.

**Alternatives considered:** a separate web-only frontend build (`VITE_TARGET=web`) — rejected because it forks the bundle and risks the two skins drifting; runtime detection keeps exactly one artifact. A separate web frontend codebase — rejected outright; it would discard the entire reuse premise of this change.

### Localhost is a trust boundary, not zero-trust

Binding `127.0.0.1` keeps the server off the network, but any web page open in the user's browser can still `fetch('http://localhost:<port>/api/invoke', …)` — and `register_workspace` + `read_artifact` would otherwise read arbitrary local files under any `openspec/changes/`. The server therefore validates the `Origin`/`Host` header against an allowlist (its own origin), and may additionally require a token that the app embeds in the URL it opens. This is the one real security decision even in the local model; it is cheap (a tower middleware layer) and contains the DNS-rebinding / cross-origin-fetch surface.

**Alternatives considered:** no origin check (rely on "it's only localhost") — rejected; localhost is reachable by every page the user visits. Full session auth — rejected as overkill for a single-user local server; an origin allowlist plus optional URL token is proportionate.

### Web-flavoured affordances reuse existing patterns

- **Workspace registration:** the native folder dialog is replaced by a text path input (sufficient on your own machine, where you know the paths) and/or a small server-side `GET /api/browse?path=` directory lister. Either way the produced string flows into the unchanged `register_workspace` command.
- **Desktop-only settings:** launch-on-login, OS notifications, and tray controls are hidden in web mode using the same convention the frontend already uses to hide the WSL poll-interval control when its command returns `null` — the web dispatch returns `null`/absent for the desktop-only queries, and the existing render path hides them.
- **Window controls:** `getCurrentWindow()` usages (show/hide/close/drag) become no-ops or browser equivalents under the host flag.

## Risks / Open Questions

- **`getCurrentWindow()` semantics in `App.tsx`** need confirming before stubbing — if it only drives window chrome (drag/close), stubs are trivial; if it drives a focus→refetch behaviour, the web path needs an equivalent (e.g. `visibilitychange`).
- **Dev ergonomics:** the cleanest dev story (Vite HMR for the web skin alongside `bun tauri dev`) needs a small proxy/port decision so it composes with the existing worktree dev-slot scheme.
- **Asset embedding vs. disk serving** is a packaging choice (`rust-embed` for a single self-contained binary vs. serving a `dist/` path); the embedded-in-desktop case must reach the bundled assets from within the Tauri process.
- **`PathBuf` arguments over JSON** (e.g. `repo_id: PathBuf`) serialize as strings identically to how Tauri already passes them, so no shape change is expected — to be confirmed per command during the dispatch-table build.
