## Context

SpecForge already owns everything this feature needs:

- **A faithful lane layout** in `graph.rs` (`layout(Vec<RawCommit>) -> CommitGraph`) that assigns rows/lanes and emits edge segments — the exact algorithm the commit-graph rail renders.
- **Commit data** via `git log` (`commit_log` returns parents + refs but only the author *name*; the leaderboard's `commit_activity_with_authors` returns the author *email* but no parents).
- **A live ref-change signal**: `CacheEvent::GraphChanged` → the `graph-changed` Tauri event, already refetched by the dashboard hook.
- **Person resolution**: the `add-author-aliases` change added a `Person` roster with `is_me` and `roster_index`; the leaderboard folds commit authors into people with you-precedence.
- **Identicon hue hashing** in `DashboardView` that turns a key into a stable colour.

What's missing is a **multi-repo, today-scoped, person-coloured** rendering of the real graph at the bottom of the dashboard. The rail is the wrong home: its spec mandates neutrality, single-repo scope, and all-history. So the garden is a separate capability that *reuses the rail's layout* but adds the multi-repo / today / person-colour concerns.

**Constraints:** Rust↔TS boundary types are hand-mirrored (`#[serde(rename_all = "camelCase")]` + `src/types.ts`); watcher/parser/git logic lives in `openspec-core`, not the Tauri crate.

## Goals / Non-Goals

**Goals:**

- A dashboard **bottom** section: one plot per top-level registered entry, stacked.
- Each plot is a **faithful** today-scoped commit graph (lanes, nodes, edges, refs, subjects) — identical topology to what the rail draws for those commits.
- Nodes **coloured by person** via the roster; "you" in the app accent.
- **Live** updates on ref changes; **re-scope at midnight**; **no persisted state**.
- Intentional **dormant** state for quiet / non-git / git-unavailable entries.
- Horizontal **overflow scroll** for wide day-graphs; **read-only**.

**Non-Goals:**

- No stylized plant / trunk / time-of-day height (the earlier garden iteration is dropped).
- No all-history or "load more" — today only.
- No commit selection or commit-detail navigation from the dashboard (that stays the rail's job).
- No growth animation — it's a graph that updates, like the rail; nothing to suppress under reduced motion.
- No persisted forest of past days; no activity-log writes.

## Decisions

### 1. New `commit-garden` capability that reuses the rail's layout

The rail's spec forbids exactly what the garden adds (semantics — per-person colour; multi-repo; today scope). Rather than bend the rail, the garden is a separate capability that calls the same `graph.rs` `layout`, so the **topology is faithful and shared** while the new concerns live in their own contract.

- *Alternative — extend the rail to multi-repo/today/coloured:* rejected; it would violate the rail spec's neutrality and single-repo clauses.

### 2. Faithful layout over today's commits

Run `graph::layout` over the current-local-day commit set. Rows, lanes, and edges come out identical to what the rail would draw for those commits. A commit whose parent predates today has that parent absent from the input, so it becomes a lane root — the "today only" framing is just a filter on `layout`'s input, and the dangling-parent problem never arises.

- *Alternative — a stylized plant abstraction:* rejected (the first iteration); the user wants the real graph.

### 3. "Today" = author-date on the local day

The day window is the viewer's current local calendar day by author date, matching the rail/heatmap day grouping. Computed from `chrono::Local`; `compute_garden` takes an explicit `today: NaiveDate` so it stays pure and timezone-test-robust.

### 4. Bottom placement

The section renders at the **bottom** of the dashboard, below the analytics overview, gated by gamification. It's a reference/observation surface, not a hero — it leads nothing, so it sits last.

### 5. Person-coloured nodes, reusing roster resolution

Each commit's author resolves to a `Person` exactly as the leaderboard does — **`is_me` first**, then `roster_index`, else the raw author, else `Unknown`. "You" gets the **app accent**; others get a **stable hue from the identicon hash** keyed on the person's primary identity. Edges stay a single neutral colour so the node colour (who) reads clearly against the topology (structure). Attribution is presentational, query-time, and never touches season scoring.

- *Alternatives:* neutral nodes like the rail (rejected — user chose per-person colour); lane-coloured edges too (rejected — busy against coloured nodes).

### 6. Pure-derived, live, zero storage

The garden is recomputed from "today's commits" on every fetch; nothing is persisted. It refreshes on `graph-changed` (reusing the existing dashboard signal) via a dedicated `useCommitGarden` hook, plus a **local-midnight tick** and a **window-focus** check so a dashboard left open or backgrounded across midnight re-scopes without user action.

- *Alternative — fold garden data into `DashboardData`:* rejected; the garden's today-scope + midnight cadence differ from the rest of the dashboard, so a dedicated command/hook avoids recomputing everything at midnight.

### 7. `commit_log_authored` carries parents, refs, and author email

No existing fetch returns parents **and** the author email **and** refs together. A dedicated `commit_log_authored` (`%H %P %an %ae %aI %D %s`, NUL-separated) supplies all three: parents and refs feed `layout`, the email feeds person resolution. The rail's `commit_log` is left untouched.

### 8. Overflow scrolls horizontally (no lane cap)

A faithful graph keeps every lane, so a wide day-graph **scrolls horizontally inside its gutter** — the rail's own overflow behaviour — rather than capping lanes or widening the dashboard. The subject column stays put.

- *Alternative — cap lanes / densify (the plant's bough cap):* rejected; capping would make the graph unfaithful.

### 9. Read-only, no selection

Nodes and rows are presentational: no commit selection, no detail navigation from the dashboard. Hover surfaces author · local time · subject (a `title`), the only metadata affordance. Commit detail remains the rail's job.

### 10. Gamification-gated

The section is part of the gamified layer (it uses the roster and lives among the gamified dashboard surfaces), so it renders and computes only when gamification is enabled, consistent with the leaderboard and season surfaces.

## Risks / Trade-offs

- **Multiple registered workspaces sharing one repo** → would duplicate plots. *Mitigation:* one plot per top-level entry, mirroring the per-repository breakdown (the view aggregation already dedupes worktrees to one repo).
- **"Today" staleness across midnight** → a stale graph. *Mitigation:* midnight tick + focus re-derive + re-derive on each render.
- **Layout cost per `graph-changed`** (lane layout per repo) → negligible: input is one day, layout is linear; bounded read (≤500 commits) before filtering.
- **Tiny day-graphs** (a repo with one linear commit today) read as a short vertical line — expected; the dormant state covers zero commits.
- **Colour accessibility** for many people → reuse the identicon palette (tuned for the dark theme); hover is the authoritative attribution, never colour alone.

## Resolved Questions

- **Placement** → bottom of the dashboard (was: top hero).
- **Fidelity** → faithful graph reusing `graph.rs` layout (was: stylized plant).
- **History scope** → today only (the deciduous framing, now a filter on a real graph).
- **Node colour** → per-person, accent for "you" (kept from the first iteration).
- **Overflow** → horizontal scroll like the rail (was: bough cap).

## Open Questions

- None blocking. Future: an optional "expand to full history" affordance per plot, or a jump-to-rail link, if the today scope proves too narrow.
