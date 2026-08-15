# Frontend

**The same bundle runs in two hosts.** `src/api.ts` gates on `isTauri()`: `invokeLogged` dispatches to Tauri's `invoke` in the desktop app and to `webInvoke` (`POST /api/invoke`) when served by `specforge-web`. So **adding a command touches four places**, not one:

1. `src/api.ts` — the wrapper
2. `crates/specforge/src/commands.rs` — the `#[tauri::command]` handler
3. `crates/specforge/src/lib.rs` — the `tauri::generate_handler![…]` list
4. `crates/specforge-web/src/dispatch.rs` — a match arm in the `/api/invoke` table

Miss step 4 and the command works in `bun tauri dev` and fails at runtime in the browser with `unknown command: X`. Neither `tsc` nor `cargo` catches it, and the served web UI is the preferred path for visual verification — so this is the failure you are most likely to hit.

Events split the same way: `listenLogged` uses Tauri `listen` in the desktop app and an `EventSource` against `/api/events` in the browser (served by `crates/specforge-web/src/sse.rs`). A new event name needs the SSE side too. Names and payload shapes are owned by `crates/openspec-app/src/events.rs`; `src/types.ts` re-declares the same string literals by hand.

**Tree selection.** The contract is the `TreeSelection` discriminated union in `src/types.ts` (nine variants); selections are emitted from `src/components/WorkspaceTree.tsx`. Adding a variant means updating three exhaustive-ish switches in `App.tsx`, not `handleSelect` (which has no switch — it delegates): `repoIdForSelection` and `renderTargetForSelection` are exhaustive, so `tsc` catches a missed arm; **`scrollAnchorForSelection` ends in `default: return null`**, so a new node type silently gets no scroll anchor unless you add its arm deliberately.
