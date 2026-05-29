## 1. Row→Y geometry in `GraphRail.tsx`

- [x] 1.1 Add a `SEP_H` layout constant and a `dayKey(commit)` / `dayLabel(commit)` helper deriving the viewer-local calendar day and a compact label (e.g. `Fri, May 29`) from the commit's existing `date`, reusing the `Date` parsing already used by `formatTimestamp`
- [x] 1.2 In a single pass over `graph.commits`, build a `rowTop: number[]` array and a `separators: { dayKey, label, y }[]` list: insert a separator (reserving `SEP_H`) above commit 0 and above any commit whose `dayKey` differs from the previous commit's; record each commit's top y in `rowTop` and the running total height
- [x] 1.3 Replace the uniform `cy(row) = row * ROW_H + ROW_H/2` with a `rowTop`-based node-center helper, and route all SVG geometry (node `cy`, edge top/bottom via `cy(band)` / `cy(band + 1)`) through it so a separator band lengthens crossing edges instead of breaking them
- [x] 1.4 Replace `height = commits.length * ROW_H` with the computed total height for both the SVG and the `.graph-rail-rows` container; position each subject row at `top: rowTop[i]` instead of `c.row * ROW_H`

## 2. Separator rendering

- [x] 2.1 Render each entry in `separators` as a non-interactive `<div className="graph-day-separator">` (not a button, no `onClick`) absolutely positioned at `top: sep.y`, `height: SEP_H`, keyed `day-${sep.dayKey}`; keep commit rows keyed by `commit.id`
- [x] 2.2 Add `.graph-day-separator` styling (label + hairline rule, dark-theme tuned) alongside the existing graph-rail styles; ensure it does not capture pointer/selection state

## 3. Verification

- [x] 3.1 `bun run build` — confirm `tsc --noEmit` (strict, `noUnusedLocals`/`noUnusedParameters`) and the Vite build pass
- [x] 3.2 `bun tauri dev` against this repo's workspace — confirm day separators appear between days, the newest day is labelled at the top, nodes stay aligned with subjects, and a lane crossing a day boundary passes through the separator unbroken (master's history has multiple commits per day to exercise both cases)
- [x] 3.3 Confirm clicking a separator selects nothing and leaves the detail pane unchanged; clicking a commit still opens its detail and restores via tree selection

## 4. Spec sync (applied at archive time via `openspec archive`)

- [ ] 4.1 Apply the `commit-graph` delta from `openspec/changes/group-commits-by-day-in-graph/specs/commit-graph/spec.md` (modifies *Faithful Commit Graph Rendering* and *Commit Selection Drives the Detail Pane*)
