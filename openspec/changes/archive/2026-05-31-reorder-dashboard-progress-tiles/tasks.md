# Tasks

## 1. Reorder the Today's Progress tiles

- [x] 1.1 In `src/components/DashboardView.tsx`, reorder the four `<HaulTile>`s in `TodayHaul` to render in this order: 🏆 `changesArchived` ("shipped"), ✚ `changesCreated` ("started"), ⎇ `commitsLanded` ("commits"), ✔ `tasksCompleted` ("tasks done"). Keep each tile's glyph, label, value, average, and the `glowTasks` glow wiring on the tasks tile intact — only the order changes.

## 2. Match the heatmap day drill-down order

- [x] 2.1 In `src/components/DashboardView.tsx`, reorder the `parts.push(...)` calls in `HeatmapDetail` so the drill-down strip lists present kinds in the same order: 🏆 shipped, ✚ started, ⎇ commits, ✔ tasks. (Each kind is still pushed only when its count is > 0; only the sequence of the `push` calls changes.)

## 3. Verify

- [x] 3.1 Run `bun run build` (tsc + bundle) to confirm no type or build regressions.
- [x] 3.2 Launch `bun tauri dev`, open the Dashboard, and confirm the Today's Progress band reads left-to-right as 🏆 shipped · ✚ started · ⎇ commits · ✔ tasks done.
- [x] 3.3 Click a populated day cell in the heatmap and confirm the drill-down strip lists its kinds in the same shipped-first order. (Verified via the auto-selected populated "today" cell, which renders through the same `HeatmapDetail` component: `🏆 7 shipped · ✚ 3 started · ⎇ 22 commits · ✔ 107 tasks`.)
- [x] 3.4 Run `openspec validate reorder-dashboard-progress-tiles --strict` to confirm the change validates.
