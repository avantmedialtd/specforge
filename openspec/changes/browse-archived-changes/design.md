## Context

SpecForge browses *active* OpenSpec changes through a master-detail tree. Archived changes (`openspec/changes/archive/<YYYY-MM-DD>-<id>/`) are fully parsed by the core and shipped to the frontend as `RepoView.archived`, but never rendered — and, worse, `compute_views` re-parses the entire archive from disk on every watcher batch (see the proposal for the full diagnosis). This change adds a way to browse archived changes *and* removes that eager-parse cost.

The shape was settled through exploration: a **global footer entrypoint** (beside Settings) that opens a **dedicated Archive view** in the detail pane, **scoped to one workspace via a dropdown**, loading on demand. This design records the technical decisions behind that shape and how it threads through the existing two-layer architecture (`openspec-core` headless + `specforge` Tauri shell + React consumer).

## Goals / Non-Goals

**Goals:**
- Make archived changes browsable and readable (read-only) without re-introducing per-batch archive parsing.
- Keep the archive entirely **off the watcher hot path** — it loads only when the Archive view is open, and only for the selected workspace.
- Reuse the established pane-swap entrypoint pattern (Dashboard header button, Settings footer button) rather than inventing tree chrome.
- Leave `WorkspaceTree.tsx` untouched; the tree stays purely about active work.

**Non-Goals:**
- Editing or un-archiving changes (the view is read-only).
- A cross-workspace / "All workspaces" pooled archive list or search (v1 is one workspace at a time).
- Surfacing canonical capability specs (`openspec/specs/`) — a separate concern.
- A persisted archive index or warm prefetch — load-on-open is sufficient for v1.

## Decisions

**1. A dedicated detail-pane view reached from a footer entrypoint — not a tree node.**
The archive is a global, retrospective, low-frequency surface; it belongs with Dashboard and Settings (the surfaces deliberately kept out of the tree), not inside the active-work tree. A footer button (`showArchive`, parallel to `showSettings`) swaps the pane to a new `ArchiveView`, with a `kind:"archive"` detail target. *Alternatives rejected:* a per-repo Archive disclosure group in the tree (reuses `DisclosureGroup`, but clutters the active tree, scales poorly past ~100 entries, and keeps the eager parse); a single global Archive node at the bottom of the tree (still in-tree, still cramped for search).

**2. Workspace-scoped via a dropdown — not a pooled cross-workspace list.**
Archive lookup is almost always "find an old change in *this* project." Scoping by workspace makes selection a first-class control, keeps the data path per-workspace (`list_archived(ws)`), and removes any need for cross-workspace aggregation or per-row workspace swatches. The dropdown only presents a real choice when ≥2 workspaces are registered. *Alternatives rejected:* a pooled global list (forces per-row mental filtering, needs a swatch column and a merged global command); workspace **tabs** (crowd/scroll past ~8 workspaces — a dropdown scales cleanly).

**3. `parse_all_archived` leaves `compute_views`; the archive loads on demand.**
The `workspace-registry` *In-Memory Cache of Parsed State* requirement governs only the active tree and badge ("source of truth for the tree pane and badge"); it says nothing about the archive. So removing the eager per-batch archive parse contradicts no requirement, and the new on-demand loading contract lives in the `archive-browser` capability (the consumer). `RepoView.archived` (full `LogicalChange[]`) is dropped from the aggregation; the Archive view sources its rows from a new command instead. Net: the hot path gets cheaper than it is today.

**4. Three loading tiers, cheapest-first.**
(a) Tree render / every watcher batch → *nothing* archive-related. (b) View open / dropdown change → a lightweight `[{ id, date, title }]` for one workspace: `id` and `date` come straight from the directory name `YYYY-MM-DD-<id>` (zero file reads), `title` from a single first-heading read of `proposal.md` (the existing title-only parse, **not** `parse_change`). (c) Row selected → that change's artifact markdown via the existing `read_artifact`, which already permits `openspec/changes/archive/…` (it is under `openspec/changes/`).

**5. Date from the directory name; reverse-chronological order.**
The `YYYY-MM-DD-` prefix the parser currently strips (`archive_dir_logical_id`) is the natural label and sort key. The listing surfaces it and orders newest-first.

## Risks / Trade-offs

- **The active↔archived diff may rely on parsed `RepoView.archived`.** → `diff_views` / `LogicalChangeArchived` must still fire when a change's last active instance archives. The per-instance `ChangeArchived` path already derives archival from a *cheap archived-id set* (the watcher's `list_archived_changes` + `archive_dir_logical_id` membership test), so the diff should be satisfiable from cheap ids without the full parse. Mitigation: if the logical diff needs the archived id set, keep that as a directory listing in the aggregation — never a `parse_change`.
- **`App.tsx` iterates `[...view.active, ...view.archived]`.** → Dropping `RepoView.archived` breaks that repo-id resolution loop. Mitigation: archived instances share worktree paths with the repo's active instances, so the lookup can resolve from `active` alone (or a cheap path→repo map); adjust the loop when the field is removed.
- **Stale archive list while the view is open.** → A change archived (or its `proposal.md` retitled) while the Archive view is open should refresh. Mitigation: re-fetch the selected workspace's list on archive events while the view is open; otherwise load lazily.
- **Dropdown default selection / single-workspace ergonomics.** → With one workspace the dropdown is degenerate (no choice); with many, which is default? See Open Questions.
- **TS/Rust type drift.** → The lightweight archive-entry type crosses IPC; add it to `types.rs` with `#[serde(rename_all = "camelCase")]` and mirror in `types.ts` in the same change.

## Migration Plan

No data migration — read-only feature. The only internal breaking change is removing `RepoView.archived` (and the `WorktreeSnapshot.archived_changes` it derives from) from the aggregation; the TypeScript mirror (`RepoView.archived`, `ChangeInstance.isArchivedHere`) updates in lockstep, and the `App.tsx` lookup adjusts. No settings schema change (the Archive view holds no persisted state in v1; `showArchive` is ephemeral UI state like `showSettings`).

## Open Questions

- **Default workspace in the dropdown** — last-viewed, most-recently-active, or the workspace currently selected in the tree?
- **In-view navigation when an archived change is opened** — replace the list with the artifact (a back affordance returns to the list), or a split (list + reading pane) within the Archive view?
- **Archived-id set retention** — does the aggregation need to keep a cheap archived-id listing for `diff_views`, or can archival be detected entirely via the existing watcher `ChangeArchived` path so the aggregation drops the archive completely?
