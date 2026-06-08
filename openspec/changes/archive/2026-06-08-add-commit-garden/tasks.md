## 1. Core: today's-graph derivation (openspec-core)

- [x] 1.1 Add `git::commit_log_authored` — a `git log --all` fetch (`%H %P %an %ae %aI %D %s`, NUL-separated, bounded by a limit) returning each commit's parents, ref decorations, full author identity (name + email), date, and subject.
- [x] 1.2 In a new `garden.rs`, filter the fetched commits to the viewer's current local day and run the existing `graph.rs` `layout` over them, yielding faithful rows, lanes, and edges (off-day parents are absent, so their commits are lane roots).
- [x] 1.3 Attribute each commit to a person with `is_me` first (you-precedence), then `roster_index`, else the raw author, else `Unknown`; carry `isMe`, a stable colour key (the person's primary identity), and a display label on each node.
- [x] 1.4 Define garden types — `GardenCommit { id, row, column, subject, refs, date, author, personKey, label, isMe }` and `WorkspaceGarden { label, dormant, commits, edges, laneCount }` — with `#[serde(rename_all = "camelCase")]`, reusing the rail's `EdgeSegment` / `CommitRef`; leave `label` empty for the shell; mark `dormant` for entries with no day commits.
- [x] 1.5 Unit-test the derivation: local-day filtering, dormant (no-commits), a branch/merge laying out as a faithful DAG (lane_count + edges), person folding with you-precedence, an unrostered author keeping a raw key, and an authorless commit falling back to `Unknown`.

## 2. Tauri shell: command + wiring (specforge)

- [x] 2.1 Add a `get_commit_garden` command returning `Vec<WorkspaceGarden>`: one entry per top-level registered item (a repository group or a flat workspace, mirroring the per-repository breakdown), calling the core derivation per repo and filling `label` from the presentation store as `get_dashboard` does, resolving "today" in the host's local time with the roster from settings.
- [x] 2.2 Gate the command on the gamification setting: when disabled, return empty and compute nothing.
- [x] 2.3 Degrade to the dormant state per entry for non-git / git-unavailable repos; never error.
- [x] 2.4 Register `commands::get_commit_garden` in the `tauri::generate_handler![]` list in `crates/specforge/src/lib.rs`; persist no new state.

## 3. Frontend: types + api

- [x] 3.1 Mirror `GardenCommit` / `WorkspaceGarden` in `src/types.ts` (camelCase), reusing the existing `CommitRef` / `EdgeSegment` types.
- [x] 3.2 Add `getCommitGarden(): Promise<WorkspaceGarden[]>` in `src/api.ts` via `invokeLogged`.

## 4. Frontend: CommitGarden component (faithful graph)

- [x] 4.1 Build a `CommitGarden` section: one stacked plot per `WorkspaceGarden`, labelled with its entry name + a commit count; render nothing when no entries are registered.
- [x] 4.2 Render each plot as a rail-style graph — a gutter SVG with lanes, nodes, and Bézier edges (reusing the rail's geometry) alongside a rows column showing ref chips + commit subject per row.
- [x] 4.3 Colour each node by person — the application accent for `isMe`, otherwise the identicon hue hash keyed on `personKey`; keep edges a single neutral colour.
- [x] 4.4 Render the dormant "quiet today" plot for `dormant` entries (quiet, non-git, or git-missing).
- [x] 4.5 Add a hover tooltip on each node/row (author · local time · subject); offer no commit selection and no mutation (read-only), and let a wide gutter scroll horizontally without moving the subject column.

## 5. Frontend: dashboard integration

- [x] 5.1 Place `CommitGarden` at the **bottom** of the Dashboard (below the analytics overview), rendered only when the gamified layer is enabled.
- [x] 5.2 Fetch garden data via a `useCommitGarden` hook independent of `get_dashboard`, refreshing on the existing `graph-changed` subscription plus a local-midnight tick and a window-focus check, so a dashboard left open or backgrounded across midnight re-scopes to the new day without user action.
- [x] 5.3 Add garden styles to `src/App.css` (section, stacked plots, gutter/edges/nodes, rows, ref chips, dormant) consistent with the dark theme and the rail's chip styling.

## 6. Verify

- [x] 6.1 `cargo test` (core + shell) green; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean.
- [x] 6.2 `bun run build` (strict `tsc --noEmit` + bundle) green.
- [x] 6.3 Run the app via `bun run wt:dev` and verify against the spec scenarios: the section renders at the bottom with one faithful graph per workspace; nodes track people (you in the accent) and refs/subjects show; a quiet workspace is dormant; clicking a node selects nothing; toggling gamification off hides the section. (Midnight re-scope is covered by the unit test in 1.5 plus code review of the 5.2 tick.)
