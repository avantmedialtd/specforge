## 1. Terminal frontend detail refresh

- [ ] 1.1 In `crates/specforge-tui/src/app.rs`, give `Msg::Artifact` a way to distinguish a selection-driven load from a watcher-driven one, and make the handler's `model.detail_scroll = 0` conditional on the former, leaving the existing `artifact_gen` staleness check untouched (`terminal-ui`: *Live Updates From the Watcher*)
- [ ] 1.2 In `crates/specforge-tui/src/app.rs`, thread that distinction through `load_selected_artifact` so every existing caller keeps its current reset-to-top behaviour
- [ ] 1.3 In `crates/specforge-tui/src/app.rs`, replace `reconcile_detail`'s early return on an unchanged selection with a watcher-driven re-read of the open artifact, keeping the selection-changed and selection-cleared paths as they are (`terminal-ui`: *The open artifact's body refreshes without a selection change*)
- [ ] 1.4 Add a `#[cfg(test)]` module to `crates/specforge-tui/src/app.rs` covering: a `Msg::Cache` with unchanged selection issues a re-read; a watcher-driven `Msg::Artifact` leaves `detail_scroll` untouched; a selection-driven one resets it to `0`; a reply whose `gen` is stale is discarded

## 2. Refresh policy module

- [ ] 2.1 Add `src/detail/refreshPolicy.ts` with the pure state and transition function from design.md — states carrying content, error, and in-flight status; events `select`, `watch`, `resolved`, `failed` — with no React, DOM, or Tauri imports
- [ ] 2.2 Implement the equality guard in the `resolved` transition so identical bytes return the previous state object unchanged (`spec-browser`: *Refresh with unchanged content is not observable*)
- [ ] 2.3 Implement the trigger-dependent policy: `watch` sets no loading flag, and a `failed` following a `watch` retains the existing content and raises no error, while `select` keeps today's loading and error behaviour (`spec-browser`: *Failed background read preserves the displayed content*, *Failed selection read still reports the error*)
- [ ] 2.4 Add `src/detail/refreshPolicy.test.ts` covering every transition in the design's state diagram, including that a `select` supersedes an in-flight `watch` and that a no-op `resolved` is referentially identical to the prior state

## 3. Detail pane wiring

- [ ] 3.1 In `src/components/DetailPane.tsx`, replace the local `content` / `error` / `loading` state with the reducer from `src/detail/refreshPolicy.ts`, keeping the existing per-run `cancelled` guard and the identity-keyed fetch effect that dispatches `select`
- [ ] 3.2 In `src/components/DetailPane.tsx`, subscribe to `onCacheUpdated` (already exported from `src/api.ts`; no new command or event is required) and dispatch a `watch` load through `useCoalescedRefetch`, unsubscribing on unmount and on target change
- [ ] 3.3 Confirm the subscription applies no condition on the event payload's `workspace` field, and record why in a comment citing the carrier behaviour of `WatcherManager::refresh_status_and_notify` (`spec-browser`: *Refresh is not conditioned on the workspace the notification names*)
- [ ] 3.4 Verify no IPC type changed, so `src/types.ts` needs no mirroring edit for this change

## 4. Scroll anchor consumption

- [ ] 4.1 In `src/components/DetailPane.tsx`, add a consumed-anchor ref and make the scroll-anchor effect act only when the current `scrollAnchor` differs by identity from the last one it consumed, recording it after scrolling — keeping `content` in the dependency list so the effect still waits for committed layout (`spec-browser`: *Reading position survives a refresh the user did not initiate*)
- [ ] 4.2 Confirm re-selecting the same Section or Task node still scrolls, since `scrollAnchorForSelection` in `src/App.tsx` returns a fresh object per selection (`spec-browser`: *Selecting a section or task still scrolls to it*)

## 5. Verification

- [ ] 5.1 Run `cargo test` for the workspace and confirm the new `specforge-tui` tests pass
- [ ] 5.2 Run `bun test` and confirm the `refreshPolicy` suite passes alongside the existing `src/routing/` suites
- [ ] 5.3 Run `bun run build` and confirm the strict `tsc --noEmit` gate passes with no unused locals or parameters
- [ ] 5.4 Smoke the desktop app with `bun run wt:dev`: open an artifact, edit its file on disk, and confirm the pane re-renders without user action
- [ ] 5.5 Continue the smoke: scroll away from the top, edit the file again, and confirm the reading position holds and no spinner appears; then select a Task node, let a further edit land, and confirm the pane does not scroll back to that task
- [ ] 5.6 Continue the smoke: touch a file in a different registered workspace and confirm the open pane does not move; then archive the open change on disk and confirm the pane keeps showing its content instead of flipping to an error
- [ ] 5.7 Smoke `specforge-tui`: open an artifact, scroll down, edit the file on disk, and confirm the body updates with the scroll offset held; then switch artifact tab and confirm it returns to the top
