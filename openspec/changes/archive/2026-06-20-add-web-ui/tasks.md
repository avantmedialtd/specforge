# Tasks

## 1. `openspec-app`: Extraction Refactor (shared by all skins)

- [x] 1.1 Move the `read_artifact` path-traversal guard (canonicalize resolved path; reject anything outside `openspec/changes/`) from `specforge/commands.rs` into `openspec-app`, with `cargo test` coverage
- [x] 1.2 Move the `CacheEvent → (name, payload)` mapping and its payload structs from `specforge/events.rs` into `openspec-app` as a pure `event_envelope(&CacheEvent) -> (&'static str, serde_json::Value)`
- [x] 1.3 Rewire `specforge/events.rs::spawn_event_forwarder` to call the shared mapping and `app.emit` the result (no behaviour change)
- [x] 1.4 Rewire `specforge/commands.rs::read_artifact` to delegate to the shared guard

## 2. `specforge-web`: New Crate (server core)

- [x] 2.1 Add `crates/specforge-web` to the workspace; deps `axum`, `tower-http`, `tokio`, `serde_json`, `openspec-core`, `openspec-app`, `rust-embed`
- [x] 2.2 `lib.rs`: `pub fn router(svc: AppService) -> axum::Router` and `pub async fn serve(svc: AppService, addr: SocketAddr)`
- [x] 2.3 `dispatch.rs`: `POST /api/invoke { command, args }` → `match command` table mapping each of the ~30 commands onto `AppService`/`SettingsStore`/`WatcherManager`, returning a JSON result or error envelope; reject unknown commands
- [x] 2.4 `sse.rs`: `GET /api/events` subscribes the `CacheEvent` broadcast and emits SSE frames via the shared `event_envelope`; handle `Lagged`/`Closed`
- [x] 2.5 Static serving: embed `dist/` via `rust-embed` for production; serve at `/`
- [x] 2.6 Tests: `router()` smoke test — a sample command round-trips; an unknown command errors; an event published to the broadcast appears on the SSE stream with the expected name/payload

## 3. `specforge-web`: Localhost Trust Boundary

- [x] 3.1 Bind `127.0.0.1` only (never a non-loopback interface)
- [x] 3.2 `tower` middleware validating `Origin`/`Host` against the server's own origin allowlist; optional token embedded in the opened URL
- [x] 3.3 Tests: cross-origin request is refused before dispatch; same-origin request passes

## 4. Entry Points

- [x] 4.1 Standalone `specforge serve` binary: bootstrap + populate an `AppService` from the shared config dir (mirroring `specforge-tui/main.rs`), then `serve()`
- [x] 4.2 Embedded toggle: settings field (enabled, port, bind) in the existing store; on enable, the desktop `lib.rs` calls `specforge_web::serve(svc.clone(), addr)` on the existing `AppService`
- [x] 4.3 Document the standalone-alongside-desktop two-writer `activity.json` contention; embedded toggle avoids it

## 5. Frontend: Transport Abstraction

- [x] 5.1 `src/api.ts`: detect host (`window.__TAURI__`); native → existing `invoke`/`listen`; web → `fetch('/api/invoke')` + `EventSource('/api/events')`
- [x] 5.2 Map SSE event names → the existing `on*` handlers so component/hook code is unchanged
- [x] 5.3 Confirm `PathBuf`-typed command args serialize identically over JSON (e.g. `repo_id`)

## 6. Frontend: Web-Flavoured Affordances

- [x] 6.1 `SettingsView`: replace the native folder dialog with a path text input (and/or a server-side `GET /api/browse?path=` directory lister) feeding `register_workspace`
- [x] 6.2 Hide desktop-only settings (launch-on-login, OS notifications, tray) in web mode using the existing "hide when query reports N/A" pattern
- [x] 6.3 Stub `getCurrentWindow()` usages in `App.tsx` under the host flag (after confirming what they drive — chrome vs. focus→refetch)

## 7. Dev / Packaging

- [x] 7.1 Dev story: serve the Vite build / proxy so HMR works for the web skin, composing with the worktree dev-slot port scheme
- [x] 7.2 Decide whether the `specforge serve` binary ships in release packaging — **decision:** the `specforge-serve` binary builds with the workspace (`cargo build`); it is NOT bundled into the Tauri desktop installer (which stays desktop-only). It can be distributed separately or run from source. Revisit if a packaged headless distribution is wanted.
- [x] 7.3 Verify the served UI renders and live-updates in a browser against a registered workspace (parity smoke test with the desktop app)
