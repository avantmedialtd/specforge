## 1. Terminal frontend detail refresh

- [x] 1.1 In `crates/specforge-tui/src/app.rs`, give `Msg::Artifact` a way to distinguish a selection-driven load from a watcher-driven one, and make the handler's `model.detail_scroll = 0` conditional on the former, leaving the existing `artifact_gen` staleness check untouched (`terminal-ui`: *Live Updates From the Watcher*)
- [x] 1.2 In `crates/specforge-tui/src/app.rs`, thread that distinction through `load_selected_artifact` so every existing caller keeps its current reset-to-top behaviour
- [x] 1.3 In `crates/specforge-tui/src/app.rs`, issue the watcher-driven re-read of the open artifact from the `Msg::Cache` arm rather than from inside `reconcile_detail`, which also runs on filter and cursor keys where an unchanged selection means nothing on disk moved (`terminal-ui`: *The open artifact's body refreshes without a selection change*)
- [x] 1.4 Add a `#[cfg(test)]` module to `crates/specforge-tui/src/app.rs` covering: a `Msg::Cache` with unchanged selection issues a re-read; a watcher-driven `Msg::Artifact` leaves `detail_scroll` untouched; a selection-driven one resets it to `0`; a reply whose `gen` is stale is discarded
- [x] 1.5 Carry the read result as `Result<String, String>` so a watcher-driven read failure retains the body already on screen instead of replacing it with the error text, and make a watcher-driven load inherit `Select` semantics when it supersedes an outstanding user-initiated load (`terminal-ui`: *A failed re-read leaves the reader's content in place*) — added during implementation because automatic re-reads make both paths newly reachable

## 2. Refresh policy module

- [x] 2.1 Add `src/detail/refreshPolicy.ts` with the pure state and transition function from design.md — states carrying content, error, and in-flight status; events `select`, `watch`, `resolved`, `failed` — with no React, DOM, or Tauri imports
- [x] 2.2 Implement the equality guard in the `resolved` transition so identical bytes return the previous state object unchanged (`spec-browser`: *Refresh with unchanged content is not observable*)
- [x] 2.3 Implement the trigger-dependent policy: `watch` sets no loading flag, and a `failed` following a `watch` retains the existing content and raises no error, while `select` keeps today's loading and error behaviour (`spec-browser`: *Failed background read preserves the displayed content*, *Failed selection read still reports the error*)
- [x] 2.4 Add `src/detail/refreshPolicy.test.ts` covering every transition in the design's state diagram, including that a `select` supersedes an in-flight `watch` and that a no-op `resolved` is referentially identical to the prior state

## 3. Detail pane wiring

- [x] 3.1 In `src/components/DetailPane.tsx`, replace the local `content` / `error` / `loading` state with the reducer from `src/detail/refreshPolicy.ts`, keeping the existing per-run `cancelled` guard and the identity-keyed fetch effect that dispatches `select`
- [x] 3.2 In `src/components/DetailPane.tsx`, subscribe to `onCacheUpdated` (already exported from `src/api.ts`; no new command or event is required) and dispatch a `watch` load through `useCoalescedRefetch`, unsubscribing on unmount and on target change
- [x] 3.3 Confirm the subscription applies no condition on the event payload's `workspace` field, and record why in a comment citing the carrier behaviour of `WatcherManager::refresh_status_and_notify` (`spec-browser`: *Refresh is not conditioned on the workspace the notification names*)
- [x] 3.4 Verify no IPC type changed, so `src/types.ts` needs no mirroring edit for this change

## 4. Scroll anchor consumption

- [x] 4.1 In `src/components/DetailPane.tsx`, add a consumed-anchor ref and make the scroll-anchor effect act only when the current `scrollAnchor` differs by identity from the last one it consumed, recording it after scrolling — keeping `content` in the dependency list so the effect still waits for committed layout (`spec-browser`: *Reading position survives a refresh the user did not initiate*)
- [x] 4.2 Confirm the guard cannot suppress a genuine re-scroll, since `scrollAnchorForSelection` in `src/App.tsx` returns a fresh object per selection — and record that section/task anchor scrolling does not currently fire at all, verified identical on the pre-change bundle, so the guard protects a dormant path (see the pre-existing defect noted in proposal.md)

## 5. Verification

- [x] 5.1 Run `cargo test` for the workspace and confirm the new `specforge-tui` tests pass
- [x] 5.2 Run `bun test` and confirm the `refreshPolicy` suite passes alongside the existing `src/routing/` suites
- [x] 5.3 Run `bun run build` and confirm the strict `tsc --noEmit` gate passes with no unused locals or parameters
- [x] 5.4 Smoke the desktop app with `bun run wt:dev`: open an artifact, edit its file on disk, and confirm the pane re-renders without user action
- [x] 5.5 Continue the smoke: scroll away from the top, edit the file again, and confirm the reading position holds and no spinner appears; then select a Task node, let a further edit land, and confirm the pane does not scroll back to that task
- [x] 5.6 Continue the smoke: touch a file in a different registered workspace and confirm the open pane does not move; then make the open artifact unreadable (`chmod 000`) and fire a watcher batch, confirming the pane keeps its content while a subsequent user-initiated selection of the same artifact still surfaces the error. Archiving the open change was tried first and does not exercise this path — the address stops resolving and the routing layer renders "Address not found" before any read fails
- [x] 5.7 Smoke `specforge-tui`: open an artifact, scroll down, edit the file on disk, and confirm the body updates with the scroll offset held; then switch artifact tab and confirm it returns to the top
