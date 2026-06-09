## Context

The Dashboard's "Recent" feed is built by `dashboard::recent_entries(views, limit)`: it iterates each repo's **active** logical changes, sorts by file `modified_at`, and truncates. Selecting a row calls `onOpenChange(worktreePath, changeId)`, which resolves the change under the live `openspec/changes/<id>/` read path.

This change repurposes that slot into "Today's ships" — the changes archived today. The relevant machinery already exists and is reused rather than rebuilt:

- **`compute_dashboard`** already fetches, per git-backed repo, a `Vec<ChangeLifecycle>` via the injected `lifecycle_for` closure (for the throughput/time-to-archive metrics). Each `ChangeLifecycle { name, created_at, archived_at }` keys on the **bare** logical id and carries the git-recovered archive instant.
- **`repo.archived`** holds the archived logical changes as lightweight stubs (`list_archived_stubs`): each stub's `change_id` is the **full dated folder name** (`2026-06-08-<id>`), with `title: None` (archive content is loaded lazily by the Archive browser).
- **`parser::archive_dir_date(dir) -> Option<&str>`** extracts the `YYYY-MM-DD` prefix; **`parser::archive_dir_logical_id(dir) -> &str`** strips it to the bare id.
- **`garden::local_today() -> NaiveDate`** already defines "today" in the viewer's local zone — shared with the commit garden so both widgets agree on the day boundary and self-clear at the same local midnight.
- The **`change-archived`** Tauri event already drives a dashboard refetch.

## Goals / Non-Goals

**Goals:**
- Replace the Recent feed with a "Today's ships" feed: changes archived today, across all workspaces, newest-archived first.
- Keep the feed alive when git is unavailable (the degradation guarantee the Recent feed currently carries).
- Show a relative archive time ("archived 2h ago") when git can supply the instant.
- Navigate a row click into the Archive browser, pre-selected on that change.
- Reuse the existing dashboard pipeline, git query, day-boundary helper, and archive event — no new Tauri command and no new git invocation.

**Non-Goals:**
- Showing the in-flight (active) changes — that list is intentionally dropped; the workspace tree already enumerates active changes and the Today's Progress hero shows the in-flight count.
- Surfacing the *capability specs* synced by an archive (the proposal scoped this to changes, not specs).
- A multi-day "recently shipped" history — strictly today, by decision.
- Gamification gating — it replaces an always-on feed and stays always-on.

## Decisions

### 1. Membership from the dated folder; the git instant is enrichment only
A change is "shipped today" **iff** its archived stub's dated folder (`archive_dir_date(stub.change_id)`) equals `local_today()`. This is the same source of truth the Archive browser shows, requires no git, and is already in memory. The git `archived_at` is consulted **only** to render the relative time on a matched row.

*Why over git-instant membership:* driving membership from `change_lifecycle().archived_at` would make the entire feed vanish when git is absent, forcing a weakening of the Dashboard's "renders without git" guarantee. Folder-date membership preserves it — without git the feed still lists today's ships, just without the "2h ago" clock.

*Trade-off:* the folder date is the archiver's local calendar day at archive time, not necessarily the viewer's. For SpecForge's single-developer, single-machine use, archiver == viewer, so they coincide. Cross-timezone shared-repo edge cases may misdate a ship by a day — accepted, because the dated folder is exactly what the user already sees in the Archive browser.

### 2. Intra-day ordering and the relative-time label come from `change_lifecycle`
For each repo, the `lifecycle_for` closure is already invoked inside `compute_dashboard`'s per-repo loop. Build a `HashMap<bare_id, archived_at>` from that same `Vec<ChangeLifecycle>` (no second git call), join each today-dated archived stub by `archive_dir_logical_id(change_id)`, and:
- order ships by `archived_at` descending (newest ship first), falling back to the dated id for a stable order when the instant is missing;
- pass `archived_at` to the frontend so it can render `relativeTime(archivedAt)`.

The ships are therefore computed **inside** the existing lifecycle loop, replacing the standalone `recent_entries(views, limit)` call that ran before it.

### 3. Titles are parsed on demand, scoped to today's ships
Stubs carry no title. Rather than eagerly parsing every archived change, inject a `ship_title_for` closure (mirroring `activity_for` / `lifecycle_for`) that `compute_dashboard` calls **only** for the handful of stubs dated today. It reads the archived change's `proposal.md` title under `openspec/changes/archive/<dated>/`. The row label is `title ?? humanized(bare_id)`. Keeping the closure injected preserves the function's testability (tests pass a stub title-resolver) and bounds the IO to the day's ships.

### 4. Repurpose the `recent` field rather than add a parallel one
`DashboardData.recent: Vec<RecentEntry>` becomes `todays_ships: Vec<ShipEntry>`; `RecentEntry.modified_at` becomes `ShipEntry.archived_at`. `ShipEntry` keeps `change_id` (the **bare** id, for navigation and display), `title`, `workspace_label`, `worktree_path`, and adds `archived_at: Option<u64>` plus the dated `archive_dir` (so the click can address the archive entry). The hand-mirrored TypeScript (`RecentEntry` → `ShipEntry`, `recent` → `todaysShips`, `modifiedAt` → `archivedAt`) changes in lockstep.

*Why a rename over keeping `recent`:* the field's meaning inverts (in-progress → shipped); a stale `recent` name would mislead every future reader of the IPC contract. It is an internal boundary with no persistence, so the rename costs only the two synchronized type definitions.

### 5. Click navigates into the Archive browser, pre-selected
The Archive browser is a standalone modal pane (`App.tsx` `showArchive` toggle, taking `workspaces`), not tree-driven, and an archived change is absent from the active `changes/<id>` read path the current Recent click uses. So a ship-row click sets `showArchive = true` and passes an **initial selection** (workspace + archived change) into `ArchiveView`, which gains an optional `initialSelection` prop and selects it on mount. This reuses the archive reader the user just built rather than adding a second archived-artifact viewer.

### 6. No new command, no new event
`get_dashboard` already returns `DashboardData`; the renamed field rides it. Liveness reuses the `change-archived` and `cache-updated` events already wired to refetch the dashboard — archiving a change makes it surface in the feed within the debounce window, and the local-midnight rollover is covered by the same day-boundary the garden uses.

## Risks / Trade-offs

- **Folder date ≠ viewer's day across timezones** → Accepted for the single-developer desktop case; membership matches the Archive browser's own dating, so the two surfaces never disagree.
- **Git instant and folder date can straddle midnight** (archived 23:59, committed 00:01) → Membership is authoritative from the folder; the git instant only labels the row. A missing or out-of-day instant degrades to "no clock," never to a wrong membership.
- **Empty most of the day** → By design — the quiet-day note mirrors the commit garden, and the two dormant states reinforce the shared "today" frame rather than reading as a bug.
- **Losing the in-flight feed** → Intentional and called out in the proposal; active changes remain visible in the tree and as the hero's in-flight count.
- **Title parse IO on the dashboard build** → Bounded to today's ships (typically 0–5 dirs); injected as a closure so it stays out of the pure compute path and is stubbed in tests.
- **Breaking IPC rename** → Internal only, no persisted data; the Rust type and its TypeScript mirror move together, and `bun run build`'s `tsc` gate catches any missed mirror.

## Migration Plan

In-process only — no persisted schema and no external consumer. The Rust `ShipEntry`/`todays_ships` rename and the TypeScript mirror land in the same change; `DashboardView` and `App`/`ArchiveView` switch over together. Rollback is a straight revert; there is no data to migrate.

## Open Questions

- None blocking. A future follow-up could let a ship row expand to the capability specs its archive synced, but that is explicitly out of scope here.
