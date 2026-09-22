# Frontend

**The same bundle runs in two hosts.** `src/api.ts` gates on `isTauri()`: `invokeLogged` dispatches to Tauri's `invoke` in the desktop app and to `webInvoke` (`POST /api/invoke`) when served by `specforge-web`. So **adding a command touches four places**, not one:

1. `src/api.ts` — the wrapper
2. `crates/specforge/src/commands.rs` — the `#[tauri::command]` handler
3. `crates/specforge/src/lib.rs` — the `tauri::generate_handler![…]` list
4. `crates/specforge-web/src/dispatch.rs` — a match arm in the `/api/invoke` table

Miss step 4 and the command works in `bun tauri dev` and fails at runtime in the browser with `unknown command: X`. Neither `tsc` nor `cargo` catches it, and the served web UI is the preferred path for visual verification — so this is the failure you are most likely to hit.

One command breaks that rule on purpose: `open_pull_request` has **no** `dispatch.rs` arm, so the web transport has no operation that opens a URL on the serving host (`web-ui`: *Link Handling in the Browser Skin*). A browser-skin pull-request row is a `target="_blank" rel="noopener noreferrer"` link instead, and `dispatch::tests` pins the command as unknown there. Don't "fix" the missing arm.

Events split the same way: `listenLogged` uses Tauri `listen` in the desktop app and an `EventSource` against `/api/events` in the browser (served by `crates/specforge-web/src/sse.rs`). A new event name needs the SSE side too. Names and payload shapes are owned by `crates/openspec-app/src/events.rs`; `src/types.ts` re-declares the same string literals by hand. The pull-request panel brought two: `pull-requests-updated` (a `CacheEvent`, payload-less — re-read with `get_my_pull_requests`) and `pull-request-panel-moved` (a direct emit from `set_bitbucket_panel_position` on both transports, carrying `{ position }`).

**The pull-request panel has four slots and no pane of its own.** `App.tsx` renders `<PullRequestPanel />` at exactly one of four JSX positions, chosen by the persisted `panelPosition` setting (`PanelPosition` in `src/types.ts`): `left-top` (between the Dashboard button and `.sidebar-tree`), `left-bottom` (the default — between `.sidebar-tree` and the Archive button), and `right-top` / `right-bottom` (first or last child of the `.rail-column` wrapper around `GraphRail`, which exists only for those two slots). The panel hides with its pane, renders nothing while the feature is disabled, and its list is height-bounded so the sidebar footer stays reachable. Its collapsed state is `localStorage` view state (`specforge.pullRequestsCollapsed`), never a setting.

**Tree selection.** The tree is two levels — a top-level row (repo group or flat workspace) and a change row — and stops there. The contract is the `TreeSelection` union in `src/types.ts`, three variants: `workspace`, `repo`, and `change` (which carries its `TreeContainer` plus the logical change name, never a worktree path). Selections are emitted from `src/components/WorkspaceTree.tsx`.

Adding a variant means updating **two** exhaustive switches in `App.tsx`, not `handleSelect` (which has no switch — it delegates): `repoIdForSelection` and `renderTargetForSelection`. Both are exhaustive over the union with no `default` arm, so `tsc` catches a missed one. There is no third switch: `scrollAnchorForSelection` and its silent `default: return null` are gone with the section and task rows, and `ScrollAnchor` is now the single kind `{ kind: "line"; line }`, produced inside `DocumentView` by the outline and by fragment links rather than by the tree.

**Which artifact and which instance is not a tree question.** A change row resolves to the change's *default artifact* of its *default instance* (`src/changeNavigation.ts` — the same module the header's tab strip reads, so a row click and the strip cannot disagree). Everything below the change — picking another artifact, picking another worktree — happens in the **change header** (`ChangeHeader` in `src/components/DetailPane.tsx`), which `App` places in live mode and `ArchiveView` places in read-only mode. Both write addresses through the existing `go()`, so history, deep links and reader windows are unaffected.
