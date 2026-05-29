# Group Commits by Day in the Graph Rail

## Why

The commit-graph rail renders one undifferentiated row per commit, newest-first, with no temporal landmarks. Scanning "what landed today vs. yesterday vs. last week" means hovering rows one at a time to read each timestamp — the rail shows *order* but never *when*. A lightweight day boundary turns the flat list into a skimmable timeline without touching the faithful topology the rail was built to preserve.

## What Changes

- The rail inserts a **day-separator row** between two consecutive commits whose author dates fall on different calendar days (viewer's local time zone). The newest day also gets a separator at the very top so the first group is labelled.
- Each separator shows the day's label (e.g. `Fri, May 29`) and is a non-interactive, non-selectable presentation row — it is not a commit, carries no node, and is skipped by commit selection.
- **Graph lanes and edges are unchanged.** The DAG topology, lane assignment, ref/tag/HEAD decorations, and edge geometry all render exactly as today. Lane edges pass straight *through* the separator band so a branch line is never visually broken by a header.
- Grouping is **calendar-day only**, by author date (`%aI` — already the field the rail carries and the key `--date-order` sorts by). No grouping by week/month, no OpenSpec semantics, no change-based grouping — the separators are a neutral time affordance, consistent with the rail's "faithful, no OpenSpec semantics" stance.
- The separator is a pure presentation concern: day boundaries are derived in the frontend from the dates already present on each `LaidOutCommit`. No new Tauri command, no new core type, no IPC change.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `commit-graph`: the *Faithful Commit Graph Rendering* requirement is amended so that, in addition to a subject per commit row, the rail groups commit rows into calendar-day sections separated by a labelled day-separator row — while preserving lane/edge/decoration faithfulness and the existing "no OpenSpec semantics" guarantee (calendar-day grouping is explicitly not an OpenSpec annotation). The *Commit Selection Drives the Detail Pane* requirement is clarified so that separator rows are non-selectable and never become a render target.

## Impact

- `src/components/GraphRail.tsx` — interleave day-separator rows among the commit rows in the right-hand subject column; derive the day key from each commit's `date`. The SVG gutter (nodes + edges) is unaffected because edges are positioned by `band`/`column`, not by DOM row order.
- `src/components/GraphRail.css` (styles currently in `src/App.css`) — a `.graph-day-separator` row style (label + hairline rule) tuned for the dark theme.
- **No backend change.** `crates/openspec-core/src/graph.rs`, `git.rs`, and the `get_commit_graph` command are untouched; `CommitGraph`/`LaidOutCommit` already carry ISO author dates.
- **Row-geometry caveat to resolve in design:** the SVG currently positions nodes/edges by `row * ROW_H` while subject rows are absolutely positioned by the same `row`. Introducing separator rows must not desynchronize the SVG lane geometry from the subject rows — design.md picks the alignment strategy (separators as overlay/zero-height vs. reserved-height bands).
