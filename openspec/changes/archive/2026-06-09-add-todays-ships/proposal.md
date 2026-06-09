# Add "Today's ships" — repurpose the Recent feed to show changes archived today

## Why

The Dashboard's "Recent" feed lists in-flight changes ordered by file modification time — information the workspace tree already surfaces, and which gives no sense of progress or completion. Repurposing that slot into "Today's ships" — the changes you *archived* today — turns a redundant feed into a daily wins shelf, completing the Dashboard's "today" story that the commit garden began (today's commits + today's ships).

## What Changes

- **Replace the Recent feed with "Today's ships":** the changes archived today, aggregated across every registered workspace, ordered newest-archived first.
- **Membership is the dated archive folder.** A change counts as shipped today when its `openspec/changes/archive/<YYYY-MM-DD>-<id>/` folder is dated to the viewer's local calendar day. This signal needs no git, so the feed still renders when git is unavailable (preserving the existing degradation guarantee).
- **Each row shows** the change title, its owning workspace/repository, and a relative archive time ("archived 2h ago"). The relative time is *enrichment* sourced from git's recovered archive instant (`change_lifecycle().archived_at`); when git is absent the row still renders, without the clock.
- **Empty state mirrors the commit garden's** quiet-day note — most days nothing has shipped yet, and that is shown honestly rather than hidden.
- **Selecting a row opens the change in the Archive browser, pre-selected** — an archived change no longer lives under the active `changes/<id>` read path, so the existing "open the proposal" navigation is replaced with a deep-link into the Archive pane.
- **Always-on, not gamification-gated** — it replaces the always-on Recent feed in the same slot, so gating it would leave a hole when gamification is off. The playful name is copy only.
- **Live** — the feed refreshes on the existing `change-archived` (and cache) events; archiving a change makes it appear in real time, and the day boundary self-clears at local midnight.
- **BREAKING (internal IPC only):** `DashboardData.recent: RecentEntry[]` becomes `todaysShips: ShipEntry[]`, with `modifiedAt` replaced by `archivedAt`. No public API; the hand-mirrored TypeScript type changes in lockstep.

## Capabilities

### New Capabilities

<!-- None. This evolves an existing Dashboard feed rather than introducing a new subsystem. -->

### Modified Capabilities

- `dashboard`: The **Recent Activity Feed** requirement is removed and replaced by a **Today's Ships Feed** requirement (changes archived today, dated-folder membership, git-enriched relative times, Archive-browser navigation). The requirements that reference the recent feed — **Reactive Dashboard Updates**, **Graceful Degradation Without Git**, and **Gamification Opt-In** (its always-on analytics list) — are updated to name the today's-ships feed instead.

## Impact

- **Rust core (`openspec-core`):** `dashboard.rs` — replace `recent_entries` (iterates `repo.active`, sorts by `modified_at`) with a today's-ships builder that selects archived changes dated to local today and joins each to the git `archived_at` already fetched for the lifecycle metrics. `types.rs` — `RecentEntry` → `ShipEntry` (`modified_at` → `archived_at`), `DashboardData.recent` → `todays_ships`. Reuses `garden::local_today`, `parser::archive_dir_logical_id`, and `git::change_lifecycle` — no new git query.
- **Tauri shell (`specforge`):** no new command or event — `get_dashboard` carries the renamed field; liveness rides the existing `change-archived` / `cache-updated` events.
- **Frontend:** `src/types.ts` mirrors the renamed types; `DashboardView.tsx` retitles the panel "Today's ships", renders the relative archive time, and swaps the empty copy; `App.tsx` routes a ship-row click to the Archive pane with an initial selection; `ArchiveView.tsx` gains an optional initial-selection prop.
