## 1. Icon Sources and Generation

- [x] 1.1 Author `public/favicon.svg` as a flat anvil glyph, tracing the geometry of `crates/specforge/icons/tray-icon.svg` into an independent file — do not import, symlink, or build-time-copy the tray asset, which `tray-indicator` governs as a pure-black macOS template (`web-app-install`: *Small Sizes Use an Authored Glyph, Not the Illustration*)
- [x] 1.2 Add `scripts/gen-web-icons.mjs` that rasterizes `crates/specforge/icons/app-icon.png` into `public/apple-touch-icon.png` (180), `public/icon-192.png` and `public/icon-512.png`, and encodes `public/favicon.ico` (16 + 32) from `public/favicon.svg`; add its image library to `devDependencies` only, so no runtime dependency is introduced (`product-identity`: *Canonical Application Icon Source*)
- [x] 1.3 Extend the script to compose `public/icon-512-maskable.png` with the illustration inset inside the platform safe area on a solid field, leaving the full-bleed icons untouched (`web-app-install`: *Icon Set Serves Masked Installers*)
- [x] 1.4 Guard the script so it writes only under `public/` — it must never modify any file under `crates/specforge/icons/`, and must never overwrite the authored `public/favicon.svg` (`product-identity`: *Canonical Application Icon Source*)
- [x] 1.5 Run the script and commit every generated asset, so the derivatives are reviewable and `dist/`-independent
- [x] 1.6 Assert on the generated `public/apple-touch-icon.png` that it is fully opaque with square corners, since the platform composites alpha onto black and applies its own mask (`web-app-install`: *Installed App Presents Its Own Icon and Window*)

## 2. Bundle Wiring

- [x] 2.1 Create `public/` and confirm `bun run build` copies its contents verbatim to stable, unhashed root paths in `dist/`, with no `vite.config.ts` change required (`web-app-install`: *Served Document Declares an Icon Set*)
- [x] 2.2 Author `public/manifest.webmanifest` with the product name, a short name, `display: "standalone"`, `start_url: "."`, `scope: "/"`, and the icon list including the maskable entry — no host, origin, or port anywhere in the file (`web-app-install`: *Web App Manifest Is Origin-Agnostic*)
- [x] 2.3 Add the head markup to `index.html`: the scalable icon, the `.ico` fallback, the Apple touch icon, the manifest link, and the standalone/status-bar meta tags (`web-app-install`: *Served Document Declares an Icon Set*, *Installed App Presents Its Own Icon and Window*)
- [x] 2.4 Declare the `theme-color` meta pair discriminated by `prefers-color-scheme`, taking the light and dark `--bg` values from `src/App.css` verbatim rather than fresh literals, and set both manifest colour fields to the dark value (`web-app-install`: *Theme and Launch Colours Come From the Design Tokens*)
- [x] 2.5 Confirm nothing in the bundle registers a service worker and that no caching layer is introduced (`web-app-install`: *Installability Adds No Service Worker*)
- [x] 2.6 Confirm no IPC command, event name, or payload shape changes, so `src/types.ts` and `crates/specforge-web/src/dispatch.rs` need no edit

## 3. Server Fallback Boundary

- [x] 3.1 In `crates/specforge-web/src/assets.rs`, define the bundle's static-asset namespace as an explicit set — the generated asset directory prefix plus a fixed list of well-known root files (the icons and the manifest) — and return `404` when a request falls inside it and no embedded asset matches (`web-ui`: *Deep-Link Durability of the Served Bundle*)
- [x] 3.2 Keep every other unmatched path on the shell fallback, and do not infer the namespace from the presence of a file extension — deep addresses are built from workspace and change identifiers that may contain dots (`web-ui`: *Deep-Link Durability of the Served Bundle*)
- [x] 3.3 Add tests to `crates/specforge-web/tests/server.rs`: a missing well-known icon path returns `404` and not an HTML body; a deep address whose identifier contains a dot still returns the shell; the manifest is served with its own content type and not shadowed by the fallback
- [x] 3.4 Confirm the existing `a_deep_address_that_matches_no_bundled_asset_is_served_the_shell`, `reloading_at_a_deep_address_still_works`, and `a_bundled_asset_path_is_served_as_itself_not_shadowed_by_the_fallback` tests still pass unmodified
- [x] 3.5 Treat these tests as the only safety net for this change: `.cargo/mutants.toml` scopes mutation testing to `openspec-core` and `openspec-app`, so a diff touching only `crates/specforge-web/` short-circuits the gate and reports green without running anything

## 4. Event Stream Recovery

- [x] 4.1 In `src/api.ts`, add lifecycle handling around the shared module-level `EventSource`: when the document is restored from a suspended or frozen state, replace the stream only if its `readyState` is `CLOSED`, and leave a still-open stream alone (`web-ui`: *Event Stream Recovers After Document Suspension*)
- [x] 4.2 After a replacement, trigger the same state re-read the frontend already performs on a `cache-updated` event, and make it single-flight so overlapping restorations collapse into one read (`web-ui`: *Event Stream Recovers After Document Suspension*)
- [x] 4.3 Confirm `crates/specforge-web/src/sse.rs` is unchanged — no event identifiers, no replay buffer, and lagging receivers still skipped rather than replayed (`web-ui`: *Event Stream Recovers After Document Suspension*)
- [x] 4.4 Confirm the desktop transport path is untouched, since the Tauri frontend receives events in-process and has no stream to restore

## 5. Verification

- [x] 5.1 `bun install && bun run build` — strict `tsc` with `noUnusedLocals`/`noUnusedParameters`, then the bundle; required once in this worktree before any `cargo` command, since `dist/` is gitignored and both `generate_context!` and `RustEmbed` need it at compile time
- [x] 5.2 `cargo test` across the workspace, plus `cargo test -p specforge-web --test server` for the fallback boundary specifically
- [x] 5.3 `cargo clippy --workspace --all-targets` clean
- [x] 5.4 Browser smoke on a debug `specforge-serve` with an isolated fake `HOME` and a scratch workspace registered via `POST /api/invoke`: assert through the DOM that the icon links and manifest link are present, that `/favicon.ico` returns an image content type rather than the shell, that `/manifest.webmanifest` parses with a relative `start_url`, and that a bogus well-known icon path returns `404`
- [x] 5.5 Re-run the same smoke against the server reached by its loopback address on a non-default port, confirming the manifest's relative `start_url` resolves to that origin and names no host
- [ ] 5.6 Desktop regression smoke via `bun run wt:dev`: window, tray, and menus behave as before and no install affordance appears (`web-app-install`: *The Install Surface Is Inert in the Desktop Shell*)
- [ ] 5.7 **Requires the user's iOS device and an active `tailscale serve`** — add the tailnet origin to the home screen, confirm the SpecForge icon and a chrome-free standalone window, then background the app, edit a workspace file on the host, resume, and confirm the UI reflects the change rather than pre-suspension state (`web-app-install`: *Installed App Presents Its Own Icon and Window*; `web-ui`: *Event Stream Recovers After Document Suspension*)
