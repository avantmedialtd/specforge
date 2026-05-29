## Context

The commit-graph rail (`src/components/GraphRail.tsx`) renders two visually-aligned columns inside a scrolling `.graph-rail-body`:

- a **sticky SVG gutter** that draws lane nodes (`<circle>`) and edges (`<path>`), and
- an absolutely-positioned **rows column** where each commit subject is a `.graph-row` button.

Both sides are keyed off the same integer `row` index from the backend layout (`LaidOutCommit.row`, `EdgeSegment.band`), via two constants:

```ts
const ROW_H = 26
const cy = (row: number) => row * ROW_H + ROW_H / 2   // SVG node/edge centers
// rows: style={{ top: c.row * ROW_H, height: ROW_H }}
```

Because the SVG and the subject rows independently multiply the *same* `row` by the *same* `ROW_H`, a node and its subject line up. The backend (`graph.rs`, `git.rs`, `get_commit_graph`) is unaware of any of this — it emits topology only, and already carries an ISO-8601 author date (`%aI`) on every commit. So day-grouping is entirely a rendering concern and needs no IPC change.

The constraint that defines this design: **any vertical space a day-separator consumes must be reflected identically on both the SVG side and the rows side**, or nodes will drift away from their subjects. The naive `row * ROW_H` mapping breaks the instant a separator adds height between two rows.

## Goals / Non-Goals

**Goals**
- Insert a labelled day-separator between commits whose author dates fall on different calendar days (viewer local time).
- Keep lanes, edges, decorations, and node/subject alignment exactly faithful — a lane alive across a day boundary passes straight through the separator band.
- Frontend-only; no backend, IPC, or type changes.

**Non-Goals**
- No grouping by week/month, no collapsible/foldable sections (that was the rejected "collapsible day sections" option), no "one node per day" summarization.
- No OpenSpec-aware grouping or tinting — separators are a neutral time affordance.
- No change to how dates are fetched or sorted; `--date-order` by author date stays the ordering authority.

## Decisions

### Decision: Reserved-height separator bands with a shared row→Y map

Replace the uniform `row * ROW_H` arithmetic on both sides with a single precomputed array that both the SVG and the rows consume.

A day-separator is inserted above commit `i` when `i === 0` (label the newest day) or when `dayKey(commit[i]) !== dayKey(commit[i-1])`. Each inserted separator reserves `SEP_H` vertical pixels. Compute, in one pass over `graph.commits`:

```ts
const SEP_H = 22
// rowTop[i] = y of the top of commit i's row, including all separators above it
let y = 0
const rowTop: number[] = []
const separators: { dayKey: string; label: string; y: number }[] = []
for (let i = 0; i < commits.length; i++) {
  if (i === 0 || dayKey(commits[i]) !== dayKey(commits[i - 1])) {
    separators.push({ ...day(commits[i]), y })
    y += SEP_H
  }
  rowTop[i] = y
  y += ROW_H
}
const totalHeight = y
```

Then derive geometry from `rowTop` instead of `row * ROW_H`:

- **Node center:** `cy(i) = rowTop[i] + ROW_H / 2`
- **Edge segment** `{ band, fromColumn, toColumn }`: top at `cy(band)`, bottom at `cy(band + 1)` — using the *same* `rowTop`-based `cy`. A separator inserted between `band` and `band+1` simply lengthens the segment; the lane is drawn straight through, satisfying the "lanes pass through the band" requirement for free.
- **Subject row:** `top: rowTop[i]`, `height: ROW_H` (unchanged otherwise).
- **Separator row:** absolutely positioned in the rows column at `top: sep.y`, `height: SEP_H`.
- **SVG + rows container height:** `totalHeight` (was `commits.length * ROW_H`).

Edges still index by `band`/`band + 1`, which are *commit* indices — they never point "at" a separator, so no edge logic changes beyond swapping the `cy` source. The separator lives only in the rows column; the gutter shows whatever lanes are alive crossing that band (often none at a day boundary, sometimes a long-running branch — both render correctly).

**Why this over the alternatives:**
- *Overlay separators (zero reserved height, floating label):* keeps `row * ROW_H` untouched, but the label overlaps a commit row and there is no band for lanes to "pass through" — it reads as a tag stuck onto a commit, not a section header. Rejected: visually muddier and weaker against the faithful-rendering requirement.
- *Backend-emitted separator rows:* would pollute `CommitGraph` with non-commit rows and force `row`/`band` indices to account for them, complicating the pure, unit-tested layout in `graph.rs`. Rejected: pushes a pure-presentation concern into the core, against the project's core/shell split.

### Decision: Group by author date in viewer-local calendar days

`dayKey` is the local-time calendar day of the commit's existing `date` (author date, `%aI`):

```ts
function dayKey(c: LaidOutCommit): string {
  const d = new Date(c.date)          // already used by formatTimestamp()
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`  // local TZ
}
```

The label is a compact `toLocaleDateString` (e.g. `Fri, May 29`). Author date is chosen because it is the field already present on `LaidOutCommit`, already rendered on hover, and the key `--date-order` sorts the rail by — so day boundaries always coincide with where rows already change date, never producing an out-of-order separator. Committer date was considered and rejected: it is not carried over IPC and could disagree with the row order, yielding separators mid-day.

Local time zone matches `formatTimestamp`'s existing `toLocaleString()` so the separator and the hover tooltip never disagree about which day a commit is on.

### Decision: Separators are inert presentation rows

The separator is a non-interactive `<div>` (not a `<button>`), carries no `onClick`, is not part of `commits`, and is never passed to `onSelectCommit`. Commit selection, the `selectedSha` highlight, and the `RenderTarget` commit variant are untouched — there is simply no code path by which a separator becomes a selection or a render target.

## Risks / Trade-offs

- **Scroll-height growth:** every distinct day adds `SEP_H`. For long histories this lengthens the scroll region modestly; acceptable and bounded by the existing windowed "load more" cap.
- **Two row types in one absolutely-positioned column:** commits and separators now coexist in `.graph-rail-rows`. Keys must stay stable (`commit.id` for rows, `day-${dayKey}` for separators) so React reconciles correctly across live refreshes.
- **`cy` is now array-indexed, not arithmetic:** a future edit that reintroduces `row * ROW_H` anywhere would silently desync the two columns. Mitigated by routing *all* geometry through the single `rowTop`-derived helper.

## Migration Plan

Not applicable — additive UI change, no persisted state, no data migration. Ships in one increment.

## Open Questions

None. The author-date / local-day / reserved-band choices are settled above.
