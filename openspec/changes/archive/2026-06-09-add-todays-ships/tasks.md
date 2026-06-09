## 1. Core data model (openspec-core)

- [x] 1.1 Replace `RecentEntry` with `ShipEntry` in `dashboard.rs` (keep `#[serde(rename_all = "camelCase")]`): fields `change_id` (bare logical id, for display and navigation), `title: Option<String>`, `workspace_label`, `worktree_path`, `archive_dir` (the dated `YYYY-MM-DD-<id>` directory name, for the Archive-browser deep-link), and `archived_at: Option<u64>` (git-recovered archive instant, replacing `modified_at`).
- [x] 1.2 Rename `DashboardData.recent: Vec<RecentEntry>` → `todays_ships: Vec<ShipEntry>`.

## 2. Today's ships builder (openspec-core)

- [x] 2.1 Add a `ship_title_for` closure parameter to `compute_dashboard` (mirroring `activity_for` / `lifecycle_for`) that resolves an archived change's title from its dated directory; default/stub it in existing call sites and tests.
- [x] 2.2 Replace the standalone `recent_entries(views, limit)` call with a `todays_ships` builder computed **inside** the existing per-repo loop (where `lifecycle_for` is already invoked): for each repo, keep the archived stubs whose `parser::archive_dir_date(change_id) == garden::local_today()`, join each to that repo's `Vec<ChangeLifecycle>` by `parser::archive_dir_logical_id` to recover `archived_at`, resolve the title via `ship_title_for`, and emit a `ShipEntry` carrying the bare id + dated `archive_dir`.
- [x] 2.3 Order the assembled ships by `archived_at` descending, falling back to a stable order (e.g. dated dir name) when the instant is absent; the feed is no longer truncated by `recent_limit` (today's set is naturally small) — drop or repurpose that parameter.
- [x] 2.4 Replace the `recent_entries` unit tests with today's-ships tests: only today-dated archives are included; ordering is newest-archived-first by instant; a missing instant yields `archived_at: None` but the entry still appears; an empty day yields an empty feed; title falls back to the bare id when `ship_title_for` returns `None`.

## 3. Tauri command wiring (specforge)

- [x] 3.1 In `commands.rs::get_dashboard`, pass a `ship_title_for` closure that parses the archived change's `proposal.md` title under `openspec/changes/archive/<dated>/`, reusing the `change_lifecycle` fetch already performed for the lifecycle metrics — no new git query and no new command.
- [x] 3.2 Confirm liveness needs no new event: the existing `change-archived` / `cache-updated` events already drive the dashboard refetch.

## 4. Frontend types (src)

- [x] 4.1 Mirror the rename in `src/types.ts`: `RecentEntry` → `ShipEntry` (`{ changeId, title, workspaceLabel, worktreePath, archiveDir, archivedAt: number | null }`) and `DashboardData.recent` → `todaysShips`.
- [x] 4.2 Verify `src/api.ts` `getDashboard` needs no signature change beyond the type mirror.

## 5. Dashboard UI (src/components/DashboardView.tsx)

- [x] 5.1 Retitle the panel from "Recent" to "Today's ships" and render its rows from `data.todaysShips`.
- [x] 5.2 Render each row's label as `title ?? changeId` and, when `archivedAt` is non-null, a relative archive time via the existing `relativeTime` helper ("archived 2h ago"); omit the time when null.
- [x] 5.3 Replace the "No active changes." empty copy with a quiet-day note mirroring the commit garden's dormant treatment.

## 6. Archive-browser navigation (src/components/ArchiveView.tsx, src/App.tsx)

- [x] 6.1 Add an optional `initialSelection` prop to `ArchiveView` (workspace + archived change) and select it on mount.
- [x] 6.2 In `App.tsx`, route a ship-row click to set `showArchive = true` and pass the matching `initialSelection`, replacing the active-change `onOpenChange` path that cannot resolve an archived change.
- [x] 6.3 Verify the selection addresses the dated archive entry so the Archive reader opens that change's artifacts.

## 7. Styling (src/App.css)

- [x] 7.1 Adapt the existing `dashboard-recent` styles to the ships rows — relative archive time treatment and the quiet-day note — renaming classes for clarity where it reads better.

## 8. Verification

- [x] 8.1 `cargo test -p openspec-core` is green (today's-ships tests and the rest of the dashboard suite).
- [x] 8.2 `bun run build` is green (the `tsc --noEmit` gate catches any missed type mirror).
- [x] 8.3 Run the app on the worktree slot (`bun run wt:dev`) and verify: today's archived changes list newest-first with relative times, clicking a row opens the Archive browser pre-selected, and a day with nothing archived shows the quiet-day note.
