# Tasks: Favorite Changes in the Workspace Tree

## 1. Settings Backend (openspec-app)

- [x] 1.1 Add `favorite_change_ids: Vec<String>` with `#[serde(default)]` to `AppSettings` in `crates/openspec-app/src/settings.rs`, plus a getter and a whole-list setter with immediate save, mirroring `set_collapsed_tree_node_ids` (`spec-browser`: *Favorite Identity and Persistence*)
- [x] 1.2 Add the *first* tests for `crates/openspec-app/src/settings.rs` (the file has no `#[cfg(test)]` module today — there is no existing collapse-set test to mirror): a favorites round-trip (set → save → reload → get returns the same list) and pre-feature compatibility (a settings JSON without the favorites field loads with an empty list and other fields intact) (`spec-browser`: *Favorite Identity and Persistence*, "Pre-feature settings file loads cleanly")
- [x] 1.3 Remove the `crates/openspec-app/src/settings.rs` line from `exclude_globs` in `.cargo/mutants.toml` — its own comment instructs deleting it the day the file gets a test; without this, mutants are never generated there and the mutation gate passes vacuously over task 1.1's lines

## 2. Command Surface (specforge, specforge-web)

- [x] 2.1 Add `get_favorite_change_ids` / `set_favorite_change_ids` `#[tauri::command]` handlers in `crates/specforge/src/commands.rs` mirroring the collapse-set command pair, and register both in the `invoke_handler` list in `crates/specforge/src/lib.rs`
- [x] 2.2 Add matching arms for both commands to the exhaustive dispatch match in `crates/specforge-web/src/dispatch.rs` (unknown commands are rejected, so the web UI cannot function without them)
- [x] 2.3 Add `getFavoriteChangeIds` / `setFavoriteChangeIds` wrappers in `src/api.ts` mirroring the collapse-set wrappers; the payload is a plain `string[]` ↔ `Vec<String>`, so no new type mirrors are needed in `src/types.ts`

## 3. Frontend State and Ordering (src/)

- [x] 3.1 Add a favorites `Set<string>` to `WorkspaceTree.tsx` with hydrate-on-mount and a 150ms-debounced write-back gated on hydration, mirroring the collapsed/expanded set effects (`spec-browser`: *Favorite Identity and Persistence*, "Rapid toggling coalesces writes")
- [x] 3.2 Derive favorite keys from the existing exported node-ID builders — `logicalChangeId(rid, name)` for repo-group changes, `changeRowId(flatWorkspaceId(uri), changeId)` for flat-workspace changes — with no new key grammar; never key on an instance-scoped id (design D2; `spec-browser`: *Favorite Identity and Persistence*)
- [x] 3.3 Apply a stable partition (favorites first, backend name order preserved within each partition) where `WorkspaceTree.tsx` maps each Repo group's logical-change array and each flat workspace's change array; no divider or header row (`spec-browser`: *Favorite-First Change Ordering*)

## 4. Frontend Star Affordance (src/)

- [x] 4.1 Render the star toggle button in a reserved slot at the extreme trailing edge of the primary line — after any existing trailing meta such as the multi-instance parent's instance-count badge, so revealing it shifts no other content — on the three favoritable row types (flattened singleton, multi-instance disclosure parent, flat-workspace change row) and not on instance child rows, in `WorkspaceTree.tsx`, with `stopPropagation` on click (chevron contract: toggling never selects the row), `aria-pressed`, an accessible label, exclusion from the tab order, and the favorite state folded into the row's treeitem-level accessible name/description (`spec-browser`: *Change-Row Favorite Toggle*)
- [x] 4.2 Add star styles to `src/App.css`: outline glyph in `--text-faint` hidden at rest and revealed while the row is hovered or holds the tree's roving focus while unstarred, solid `--accent` glyph with no glow always visible while starred, composing with the existing row hover and selection treatments (`spec-browser`: *Change-Row Favorite Toggle*; `visual-identity`: *Accent Color*, *Outlined Chip Badges*)
- [x] 4.3 Add Cmd+D (macOS) / Ctrl+D (Windows, Linux) to the tree keydown switch in `WorkspaceTree.tsx`: toggle the focused favoritable row's favorite state with `preventDefault` (the browser's bookmark chord must not fire in the served web UI); inert on all other row types; add a platform-command-modifier guard to the typeahead branch so the chord's letter never moves typeahead focus (`spec-browser`: *Change-Row Favorite Toggle*, keyboard scenarios)

## 5. Verification

- [x] 5.1 `bun install && bun run build` — strict tsc (`noUnusedLocals`/`noUnusedParameters`) then bundle; must run before cargo in a fresh worktree because `specforge-web` embeds `dist/`
- [x] 5.2 `cargo test` across the workspace
- [x] 5.3 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff` — no surviving mutants on the changed `openspec-app` settings lines (task 1.2's tests kill them; task 1.3 makes them visible to mutants). The specforge / specforge-web command arms are permanently outside mutants scope (`.cargo/mutants.toml` excludes those crates as unbuildable in the scratch tree) and are covered by `cargo test` and the smoke instead
- [x] 5.4 Manual smoke via `bun run wt:dev` walking the spec scenarios: star and unstar a change in a repo group and a flat workspace; verify quiet-float ordering and name order within partitions; verify toggling never changes selection or the detail pane; verify Cmd+D on a focused change row and its inertness elsewhere (including that the `d` does not trigger typeahead); restart the app and verify persistence; verify instance child rows carry no star; add a second worktree instance of a starred singleton and verify the promoted disclosure parent stays starred (then remove it and verify the flattened row is still starred); archive a starred change on disk, verify it leaves the tree, un-archive it, and verify the star returns; navigate via addresses including Back/Forward and verify no favorite state changes
