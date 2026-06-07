# Browse Archived Changes

## Why

The application can only browse *active* changes. The moment a change is archived it leaves the UI — and nothing else surfaces it either, even though the core fully computes it. Two findings shaped the design, and the second is the more important one.

**1. The archive is computed and shipped to the client, then never surfaced.** `parser::parse_all_archived` parses every archived change; the aggregator splits each repository into `active`/`archived` (`repo_view.rs:60-64`); that `archived` array crosses the IPC boundary into `RepoView.archived`. But nothing renders it — `WorkspaceTree.tsx` references `archived` nowhere, and there is no other archive surface. The work reaches the frontend and is dropped.

**2. Worse, the archive is re-parsed in full on every watcher batch — for data nobody sees.** Active changes live in the passive `WorkspaceCache` and are maintained incrementally. Archived changes are **not cached**: `compute_views` calls `parse_all_archived` for every worktree on every aggregation (`repo_view.rs:206`), and `compute_views` runs from `refresh_aggregated_view` on **every debounced filesystem batch** (plus startup and every register/unregister). Each call does a *full* `parse_change` per archived entry — read `proposal.md` for its title, parse the whole `tasks.md` into sections and task lines, scan `specs/`:

```rust
// repo_view.rs — inside compute_views, per worktree, every batch:
let archived_changes = parse_all_archived(&entry.folder).unwrap_or_default();
```

So ticking one checkbox in an *active* change re-parses all **47** archived changes from disk — feeding only the unrendered `RepoView.archived`. At hundreds of entries the cost compounds every batch, forever.

The chosen design resolves both at once: the archive becomes a **dedicated, on-demand surface** — a footer entrypoint, beside Settings, that opens an **Archive view** in the detail pane — loaded *only when opened*, and *only for the workspace the user picks*. That removes the eager per-batch parse from the hot path entirely and surfaces the archive in a place built to scale.

## Placement & Shape

The Archive entrypoint is **global** — a single footer button, not a per-repo tree node — mirroring the surfaces the app already keeps *out* of the workspace tree:

- **Dashboard** — a header entrypoint (`sidebar-header-button`) that swaps the detail pane (`kind:"dashboard"`).
- **Settings** — a footer entrypoint (`sidebar-footer-button` + `showSettings`) that swaps the detail pane.

The Archive joins them as a **third pane-swap entrypoint**: a labelled footer button (🗄 Archive) directly above Settings, with the same toggle semantics Settings already has. The workspace tree is **untouched**; it stays purely about active work.

But unlike the Dashboard (which aggregates across workspaces), the Archive **view is scoped to one workspace at a time**, chosen from a dropdown — because archive lookup is almost always *"find an old change in this project,"* not *"scan everything at once."* Scoping by workspace makes the selection a first-class control instead of a per-row label to mentally filter, and it keeps the data path per-workspace (`list_archived(ws)`) with no cross-workspace aggregation.

```
  left sidebar                    detail pane — Archive view
  ┌────────────────────────┐     ┌──────────────────────────────────┐
  │ [▣ Dashboard]  header   │     │  Workspace: [ specforge ▾ ]       │
  │ ▾ specforge             │     │  ────────────────────────────    │
  │    add-seasonal-…       │     │  🔎 search this archive…          │
  │    …  (active work only)│     │  2026-06-07 · remove-milestones   │
  │  ─────────────────────  │     │  2026-06-07 · remove-dash-toggl…  │
  │ [🗄 Archive]  footer ◀NEW│    │  2026-06-06 · add-developer-pro…  │
  │ [⚙ Settings]  footer    │     │  …  newest first                  │
  └────────────────────────┘     └──────────────────────────────────┘
```

## What Changes

- **A footer Archive entrypoint.** A `sidebar-footer-button` above Settings with its own `showArchive` toggle (parallel to `showSettings`) and a `kind:"archive"` detail target. Label-only — no count badge — so nothing about the archive needs computing until the view is opened.

- **A workspace-scoped Archive view in the detail pane.** A new `ArchiveView` component: a **workspace dropdown** at the top, a search box, and a flat, **newest-first** list of the selected workspace's archived changes, each row `YYYY-MM-DD · <id/title>` (no workspace swatch needed — the dropdown already establishes scope). Search filters case-insensitively on id + title within the selected workspace. The dropdown defaults to a sensible workspace and only presents a real choice when ≥2 workspaces are registered; with one workspace it simply shows that archive (the default-selection rule — last-viewed vs most-recently-active vs tree-selected — is a design detail for the design phase).

- **On-demand, tiered, per-workspace loading** — the efficiency core:

  | When | Loads | Cost |
  | --- | --- | --- |
  | Tree render / every watcher batch | nothing archive-related | **zero** |
  | Archive view opened / dropdown changed | `[{ id, date, title }]` for **one** workspace | id/date from dir names (no file reads); title = one first-heading read per change — **not** the full `parse_change` |
  | Archived change selected | that change's artifact markdown | `read_artifact` — already lazy, already permitted under `openspec/changes/` |

- **`parse_all_archived` leaves the hot path.** `compute_views` stops eagerly parsing the archive on every aggregation, and the full per-repo `RepoView.archived` (`LogicalChange[]`) is no longer needed for rendering — the Archive view sources its data from the per-workspace command instead. If the active↔archived diff still needs to know an id archived, that is a cheap directory listing, not a parse. The hot path ends up **lighter than it is today**.

- **A per-workspace on-demand listing command** — `list_archived(workspaceUri) -> [{ id, date, title }]`, invoked when the view opens and whenever the dropdown changes; never on the watcher's hot path. No cross-workspace aggregation. The result may be memoized per workspace and invalidated on archive events while the view is open.

- **Reading an archived change.** Selecting a row navigates the detail pane to that change's artifacts via the existing read-only rendering path. The exact in-view navigation (list → artifact → back, vs a split within the view) is a design detail deferred to the spec/design phase.

- **Live archival.** `LogicalChangeArchived` already fires and triggers a refetch; while the Archive view is open and showing the affected workspace it re-fetches that workspace's list. Otherwise there is nothing to do.

## Capabilities

### Added Capabilities

- A new capability for the **Archive view** (working name `archive-browser`): the footer entrypoint, the workspace-scoped view (workspace dropdown + search + newest-first list), on-demand per-workspace tiered loading, and read-only artifact navigation from a selected archived change.

### Modified Capabilities

- `spec-browser`: a footer **Archive entrypoint** above Settings, mirroring the existing *Settings Entrypoint in Sidebar Footer* requirement (toggle semantics, active-state treatment, pane swap, no floating button). The deferred *"Archive section (if rendered)"* hedge in *Workspace Tree Hierarchy* is resolved the other way — archived changes are surfaced in the dedicated Archive view, **not** the tree; the tree stays active-only.
- The parser / aggregation layer that owns `parse_all_archived` and `RepoView`: the per-batch aggregation **no longer parses the archive**; archived content is exposed only through the per-workspace on-demand listing, ordered date-descending, with the date surfaced from the directory name.

## Impact

- **Spec:** a new `archive-browser` spec (the view, its entrypoint, the workspace dropdown, loading, search, read-only navigation); a `spec-browser` modification (footer Archive entrypoint; tree stays active-only); a modification to the aggregation capability (archive off the hot path, lazy, per-workspace, date-descending, date surfaced).
- **Code (frontend):** a new `ArchiveView` component (workspace dropdown + search + list); the footer button + `showArchive` state + `kind:"archive"` detail target in `App.tsx`; an `api.ts` wrapper for the listing command. **`WorkspaceTree.tsx` is untouched** — and the small `App.tsx` lookup that currently iterates `[...view.active, ...view.archived]` adjusts when `RepoView.archived` is dropped.
- **Code (backend):** `commands.rs` — a per-workspace on-demand `list_archived` command; `parser.rs` — a lightweight `{ id, date, title }` listing (dir-name id+date, title-only proposal read); `repo_view.rs` — drop `parse_all_archived` from `compute_views` (cheap ids only if the diff requires); `types.rs` + `types.ts` — the lightweight archive-entry type with `#[serde(rename_all = "camelCase")]`. `read_artifact` unchanged.
- **Behaviour delta:** the archive becomes browsable in a dedicated, searchable, per-workspace view; the tree and the watcher hot path get **cheaper** (no eager archive parse); active-work navigation is unchanged.
- **Risk:** low. The workspace tree is untouched; the efficiency change *removes* work from the hot path and defers the rest behind explicit user intent; the new surface reuses the established pane-swap entrypoint pattern (Dashboard, Settings). The main thing to verify when implementing: the active↔archived diff (`diff_views` / `LogicalChangeArchived`) is satisfied by cheap ids rather than the full parse — the per-instance `ChangeArchived` path already derives archival from a cheap archived-id set, so this is expected to hold.

## Out of Scope

- **A cross-workspace / "All workspaces" pooled archive view or search.** v1 scopes to one workspace at a time via the dropdown; a unified cross-workspace archive can come later.
- **Editing or un-archiving from the UI.** The Archive view is read-only, consistent with *Read-Only Viewer*.
- **Surfacing canonical capability specs** (`openspec/specs/<capability>/`). That is the *other* "spec browser" gap surfaced during exploration — a distinct change with its own parser and read-gate work.
- **A persisted archive index or warm prefetch.** v1 loads the per-workspace list on open / selection (optionally memoized); a persistent index is a later optimization only if latency proves noticeable.
- **A count badge on the footer Archive button.** Omitted so nothing archive-related is computed until the view opens; revisitable if a count proves wanted.
