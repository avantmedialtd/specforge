# Add the commit garden — per-workspace today commit graphs at the dashboard bottom

## Why

The dashboard shows *what* you've shipped as aggregates — heatmap cells, leaderboard rows, a streak counter — but nothing shows **today's actual commit shape across your workspaces**. SpecForge already extracts the full commit DAG and emits a live `GraphChanged` signal, yet that liveness only feeds a single-repo, all-history rail tucked to the far right. There's an opportunity for a glanceable, per-workspace view at the bottom of the dashboard: each registered workspace's *today* commits drawn as a faithful little git graph, with nodes coloured by who committed, refreshing live and re-scoping at midnight.

It reuses the rail's faithful lane layout (so the graphs match `git log --graph`) but adds two things the rail doesn't: it is **multi-repo** (one plot per workspace) and **today-scoped**, and it tints nodes by **person** via the roster. It sits in the dashboard's gamified layer alongside the leaderboard.

## What Changes

- Add a **commit-garden section** to the **bottom** of the dashboard: one plot per top-level registered entry (a repository group or a flat workspace, mirroring the per-repository breakdown so multiple worktrees of one repo share a plot), stacked vertically and labelled with the entry's display name.
- Each plot is a **faithful today-scoped git graph** — the same lane layout the commit-graph rail uses, run over that workspace's current-local-day commits: one node per commit in a lane, edges for branch/merge topology, ref decorations and commit subjects per row. Commits whose parents predate today are lane roots.
- Nodes are **coloured by person**: each commit's author is resolved to a `Person` via the roster (`is_me`-first, then `roster_index`), reusing the developer-identity resolution shipped in `add-author-aliases`. "You" carries the app accent; other people get stable hues; an authorless commit falls back to `Unknown`.
- The graphs **update live**: new commits appear within the watcher's debounce window, driven by the existing `GraphChanged` event, and re-scope to the new day at local midnight (re-derived on render / graph-change / focus / midnight tick) without user action. No history is persisted — the view is purely derived, so **no new storage and no migration**.
- A workspace with no commits today renders an intentional **dormant "quiet today"** plot, not an error or blank. Non-git (flat) workspaces and a missing `git` binary degrade to the same dormant state; the rest of the dashboard is unaffected.
- A wide day-graph **scrolls horizontally** inside its gutter (like the rail) rather than widening the dashboard. The section is **read-only** — no commit selection or detail from the dashboard.

## Capabilities

### New Capabilities

- `commit-garden`: A dashboard section, at the bottom of the dashboard, that renders per top-level registered entry (repository group or flat workspace, deduped so worktrees of one repo share a plot) a faithful today-scoped commit graph with nodes coloured by the committing person, updating live on ref changes and re-scoping at local midnight; degrades to a dormant state for quiet, non-git, or git-unavailable entries; read-only.

### Modified Capabilities

- `dashboard`: The commit garden joins the gamification-gated layer (rendered only when gamification is enabled, computed only then).

## Impact

- **openspec-core**: new derivation of "today's commit graph per repo" — `commit_log_authored` (a `git log` fetch carrying parents, refs, and the author email) feeds the existing `graph.rs` lane layout filtered to the current local day; identity resolution (`is_me`, `roster_index`) attributes each node to a `Person`. New types cross the IPC boundary (`#[serde(rename_all = "camelCase")]`, mirrored by hand in `src/types.ts`); the laid-out graph reuses the rail's `EdgeSegment` / `CommitRef`.
- **specforge (Tauri shell)**: a `get_commit_garden` command (gamification-gated, per top-level entry, labels joined from the presentation store); no new persisted state.
- **Frontend**: a `CommitGarden` component (rail-style SVG lanes/nodes/edges, person-coloured nodes) placed at the dashboard bottom; a `useCommitGarden` hook that refreshes on `graph-changed` plus a midnight tick and window focus.
- **No migration / no new on-disk files**: pure-derived. `activity.json`, settings, and presentation are untouched.
- Reuses the People roster from `add-author-aliases` as the node-colour key — no change to developer-identity's contract.
