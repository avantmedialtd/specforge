## 1. App layer: the setting and its event

- [x] 1.1 In `crates/openspec-app/src/settings.rs`, add `commit_history_enabled: bool` to `AppSettings` under `#[serde(default = "default_commit_history_enabled")]` with a `default_commit_history_enabled() -> bool { true }` beside `default_notifications_enabled`, and list it in `impl Default`. Never use a bare `#[serde(default)]`: it loads `false` and would switch every existing installation's graph off (`commit-graph`: *Commit History Can Be Turned Off*).
- [x] 1.2 In `settings.rs`, add `SettingsStore::commit_history_enabled()` and `set_commit_history_enabled(bool) -> io::Result<()>`, persisting the same way `set_document_width` does. In its `tests` module, mirror the reading-width tests: it defaults to on, it round-trips through disk (`true` and `false`, re-read from a fresh store), and a settings file without the key loads it on while keeping its neighbours.
- [x] 1.3 In `crates/openspec-app/tests/workspace_management.rs`, extend the bootstrap-from-file test, or add one beside it, so that `AppService::bootstrap` over a settings file lacking `commitHistoryEnabled` reports `svc.settings.commit_history_enabled()` as `true`. This is the test that kills the `default_commit_history_enabled -> false` mutant.
- [x] 1.4 In `crates/openspec-app/src/events.rs`, add `pub const EVENT_COMMIT_HISTORY_ENABLED_CHANGED: &str = "commit-history-enabled-changed";` with a doc comment following `EVENT_DOCUMENT_WIDTH_CHANGED`'s: a direct emit raised by a command, never a `CacheEvent`, on both transports, carrying the new `bool`. Extend the existing event-name test so the name is not a cache-event name and differs from every other constant. Mirror the string in `src/types.ts`.

## 2. Desktop shell and web server

- [x] 2.1 In `crates/specforge/src/commands.rs`, add `get_commit_history_enabled` and `set_commit_history_enabled(enabled: bool)`. The setter persists through the store, then calls `app.emit(EVENT_COMMIT_HISTORY_ENABLED_CHANGED, enabled)`, following `set_document_width`. Register both in `crates/specforge/src/lib.rs`'s `tauri::generate_handler![…]`.
- [x] 2.2 In `crates/specforge-web/src/dispatch.rs`, add the `get_commit_history_enabled` and `set_commit_history_enabled` arms. The setter sends `(EVENT_COMMIT_HISTORY_ENABLED_CHANGED, enabled)` on `extra_tx`, as the `set_document_width` arm does. Add `set_commit_history_enabled_emits_the_change_event` beside `set_document_width_emits_the_change_event`. Add a stream test in `crates/specforge-web/src/sse.rs` beside `document_width_event_appears_on_stream`, so the browser transport is pinned too (`commit-graph`: *Commit History Can Be Turned Off*).

## 3. Frontend: the switch

- [x] 3.1 In `src/api.ts`, add `getCommitHistoryEnabled()`, `setCommitHistoryEnabled(enabled)` and `onCommitHistoryEnabledChanged(handler)` (a `listenLogged<boolean>` on the new event), importing the event name from `src/types.ts`.
- [x] 3.2 Create `src/commitHistory.ts`, a pure module (see `docWidth.ts`'s header for why: no component tests, and the mutation gate skips `src/`). It exports:
  - the mirror key `specforge.commitHistoryEnabled`;
  - `readMirroredCommitHistory(store?)`, where absent, unrecognised or a throwing store all mean on;
  - `writeMirroredCommitHistory(value, store?)`, which swallows storage errors;
  - `railHasOccupant(historyOn, panels)`, equal to `historyOn || paneTakesReserve("rail", panels)`.

  Add `src/commitHistory.test.ts` covering the mirror's absent/`"false"`/garbage/throwing cases and a write→read round trip. Cover the occupancy truth table too: history on; a present panel in the rail; a present panel only in the sidebar; a panel positioned in the rail but not present; a `null` position (`spec-browser`: *Rail Exists Only While Occupied*).
- [x] 3.3 Create `src/hooks/useCommitHistoryEnabled.ts` with the `useDocumentWidth` shape:
  - state initialised from the mirror;
  - reconciliation with `getCommitHistoryEnabled()` on mount, where a failed read keeps the mirrored value;
  - adoption of `onCommitHistoryEnabledChanged`;
  - a setter that applies and mirrors immediately, then persists.

  Return `[enabled, choose]`.
- [x] 3.4 In `src/components/SettingsView.tsx`, add a *Commit history* section beside *Reading width* with one switch, taking `commitHistoryEnabled` / `onCommitHistoryEnabledChange` props as the reading width does. Render it on both hosts, outside any desktop-only gate. In one or two quiet sentences, say that turning it off removes the commit graph and its git reads, and that pull-request panels placed in the rail keep it (`commit-graph`: *Commit History Can Be Turned Off*).

## 4. Frontend: pull-request snapshots move into `App`

- [x] 4.1 Create `src/hooks/usePullRequestSnapshot.ts`, returning `PanelSnapshot | null` per provider. Lift the read-and-subscribe effect out of `PullRequestPanel`: `PANEL_SOURCES[provider].fetch()` on mount and on each `PANEL_SOURCES[provider].onUpdated` event, with the same unmount guard.
- [x] 4.2 In `src/components/PullRequestPanel.tsx`, make the panel render a `panel: PanelSnapshot | null` prop instead of fetching. Remove `onPresenceChange` and its effect. Reset `nowMs` whenever the snapshot changes, so relabelling keeps its current cadence. Export a pure `panelPresent(panel)`, which is `panel !== null && panelBodyState(panel) !== null`, and test it in `PullRequestPanel.test.ts` for `null`, disabled, unauthenticated, unavailable, ok-empty and ok-with-rows. The existing pure decisions and their tests stay as they are.
- [x] 4.3 In `src/App.tsx`, call `usePullRequestSnapshot` for both providers and pass each snapshot to its `PullRequestPanel` in `panelsAt`. Replace the `bitbucketPresent` / `githubPresent` state and setters with values derived through `panelPresent`, and feed them to the existing `panelStates` / `paneTakesReserve`. The sidebar's reserve behaviour must not change (`spec-browser`: *Side Panes Host the Pull-Request Panel*).

## 5. Frontend: the rail exists only while occupied

- [x] 5.1 In `src/App.tsx`, read `const [commitHistoryEnabled, chooseCommitHistory] = useCommitHistoryEnabled()` and compute `railOccupied = railHasOccupant(commitHistoryEnabled, panelStates)`. Pass `far={null}` while unoccupied. While occupied, `far` is:
  - with history on, today's composition, either the bare `graphRail` or the `.rail-column` wrapping it;
  - with history off, a `.rail-column.rail-column--no-graph` holding `panelsAt("right-top")` then `panelsAt("right-bottom")` and no `.rail-column-graph`.

  `SplitPane` needs no change (`spec-browser`: *Rail Exists Only While Occupied*; `commit-graph`: *Commit-Graph Rail Pane*).
- [x] 5.2 In `src/App.tsx`, gate the graph fetch on both conditions: `useCommitGraph(railHidden || !commitHistoryEnabled ? null : graphRepoId, graphLimit)`. `applyGraphRepoId` keeps tracking the selection either way, so turning history on fetches the current repository (`commit-graph`: *Commit History Can Be Turned Off*).
- [x] 5.3 In `src/App.tsx`, keep a `railOccupiedRef` updated every render. Both the `onToggleCommitRail` listener (macOS desktop) and the keydown handler's Alt branch (every other surface) return early while it is false, without flipping `railHidden`. The sidebar binding is unaffected (`spec-browser`: *Side-Pane Visibility Toggles*; `application-menu`: *View Submenu Pane Toggles*).
- [x] 5.4 In `src/App.tsx`, pass `commitHistoryEnabled` and a handler to `SettingsView`. The handler calls `chooseCommitHistory(next)` and, when `next` is true, `setRailHidden(false)` on this surface only. Turning it off leaves visibility alone (design D7).
- [x] 5.5 In `src/App.css`, under `.rail-column--no-graph`:
  - lift `.pull-request-list`'s `max-height: 40vh`;
  - let expanded panels share the column's height, each list scrolling internally;
  - keep a collapsed panel's header from shrinking.

  Check that the hairline rules for `.rail-column > .pull-request-panel:first-child` and its sibling still read correctly with no graph between the two slots (`spec-browser`: *Side Panes Host the Pull-Request Panel*).
- [x] 5.6 In `src/CLAUDE.md`, make three updates:
  - Rewrite the *Two pull-request panels, four slots* paragraph: snapshots are read in `App` by `usePullRequestSnapshot` and presence is derived with `panelPresent` (`onPresenceChange` is gone); the rail exists only while `railHasOccupant` is true (`far={null}` otherwise), and its toggles are guarded, not state-flipping, while absent.
  - Add the `get_` / `set_commit_history_enabled` pair and the `commit-history-enabled-changed` event (both transports, direct emit) to the events paragraph.
  - State that commit history is a setting while rail visibility stays view state.

## 6. Verification

- [x] 6.1 Run `bun install && bun run build` once in the fresh worktree, then confirm `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test` are green.
- [x] 6.2 Confirm `bun run build` and `bun test` are green. Root `bun test` discovery grows by the new `src/commitHistory.test.ts`; that is expected and is not the `bunfig.toml` trap.
- [x] 6.3 Mutation-test the diff: `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Add assertions for any survivor in `settings.rs` or `events.rs`.
- [x] 6.4 Manual smoke in the browser loop: a debug `specforge-serve` serving the rebuilt `dist/` with isolated state, plus `bun run dev`. Walk the scenarios of every delta:
  - with history on by default, the rail renders exactly as before, including the Dashboard's "No repository" placeholder;
  - turning history off removes the rail, its divider and any restore chevron, and Cmd/Ctrl+Alt+B then does nothing;
  - with history off, selecting nodes in different repositories issues no `get_commit_graph` invoke (check the network log);
  - a reload with history off paints no rail at any point;
  - enabling a provider whose panel sits at Rail top brings the rail back holding only that panel, at full height and scrolling internally;
  - both panels in the rail stack in slot order and share the height; a collapsed panel keeps its header;
  - hiding a rail that holds only a panel and then disabling that provider removes the restore chevron;
  - turning history on while the rail was hidden shows it;
  - a second connected tab adopts each switch change over SSE without a reload.
- [x] 6.5 Manual smoke with `bun run wt:dev` on macOS:
  - View → Toggle Commit Rail with no rail shows the window and toggles nothing;
  - with a rail, it toggles exactly once per press;
  - after turning history off and relaunching, the first frame has no rail.
