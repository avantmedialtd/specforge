# Design

## Context

The Dashboard grew in two layers. The original analytics — summary metrics, a
commits-per-day chart, lifecycle figures and a per-repository breakdown — were
later demoted beneath a progress band (haul tiles, a year-long contribution
heatmap, a streak, a leaderboard, a commit garden). The demotion moved the older
layer down the page but did not reconsider what belonged in it, and the source
still labels the band *"Demoted analytics (existing snapshot, now below the
progress band)"*.

Two facts, both established by reading the assembly rather than the rendering,
shape every decision below.

The chart and the heatmap are scoped to different populations. `service.rs`
filters the heatmap's commit days through `is_me`, while the chart's dates pass
through `activity_dates_since` unfiltered. The chart is therefore *everyone's*
commits and the heatmap is *the developer's* — a distinction invisible on a solo
repository and material on a shared one.

The fourteen-day constant is shared. `DASHBOARD_ACTIVITY_WINDOW_DAYS` bounds the
chart's cutoff at one call site and `lifecycle_metrics` at another, so removing
the chart does not remove the constant — it reveals that the constant was
misnamed.

The terminal frontend is the control case: it renders Summary → Ships today →
Activity → Leaderboard, with no chart and no breakdown, and has not been
reported as missing anything.

## Goals / Non-Goals

**Goals**

- Remove the Overview band's dead space without leaving an unread payload behind.
- Make the breakdown's order answer *"where is my work"* rather than *"what did
  I register first"*.
- Give the band a height that does not vary with the size of the registry.
- Converge the desktop's section order with the terminal's.

**Non-Goals**

- Reworking the progress band. The haul tiles, heatmap, streak, leaderboard and
  commit garden keep their present content and behaviour.
- Making the breakdown interactive. The Dashboard is read-only, and rows do not
  become navigation targets in this change.
- Changing what lifecycle mining costs or when it runs.
- Adding a team or multi-author view to replace the signal the chart carried.

## Decisions

### Remove the chart rather than repair it

Every defect in the chart is fixable — a baseline, capped bar widths, a visible
zero-day stub, an emphasised final column, a leading total. Fixing them yields a
well-drawn chart whose unique contribution, over the heatmap two sections above
it, is per-day magnitude for one fortnight. That is a small return for a card
occupying half the band, and the terminal frontend has shipped without it
throughout.

**Rejected: polish the bar strip in place.** It keeps the band at two cards and
solves the proportions, but it spends the band's left half re-answering a
question the heatmap answers over a window twenty-six times longer.

**Rejected: replace it with a throughput chart** plotting commits against
archives so the lifecycle figures beneath it are finally visualised. The most
interesting option and the largest: it needs a second series in the payload, a
dual encoding, and a legend, to explain two numbers that fit comfortably on one
line of text.

### Delete the payload, do not leave the field

With the card gone, `activity` and `ActivityBucket` have no reader in any
frontend. A field no consumer reads still crosses the IPC boundary, still needs
its hand-written mirror in `src/types.ts` kept in step, and — in `openspec-core`
— still falls inside the mutation gate, where its lines must be covered by
assertions that no longer defend any rendered behaviour.

The window constant is the exception. It bounds the lifecycle metrics
independently of the chart, so it survives, renamed from
`DASHBOARD_ACTIVITY_WINDOW_DAYS` to `DASHBOARD_LIFECYCLE_WINDOW_DAYS`, with the
payload field renamed alongside it. The rename is the point: without it the
surviving field's name advertises a chart that no longer exists.

**Rejected: keep the payload for a future team view.** Speculative, and the data
is cheap to reintroduce — `activity_dates_since` is a filter over a walk the
heatmap already performs, so nothing about restoring it later requires the code
to have been kept.

### Rank and cap at five, rather than filter to active

Rows are ordered by the composite key

$$k_i = \bigl(-a_i,\ -h_i,\ \ell_i\bigr)$$

taken ascending, for active count $a_i$, archived count $h_i$ and label $\ell_i$.
All three components are load-bearing: in a registry where most repositories are
quiet, $a_i$ ties at zero for the majority and $h_i$ becomes the effective
order, while $\ell_i$ is what stops two repositories with identical counts from
trading places between refreshes.

The list shows at most $N = 5$ entries and closes with a remainder line.
Filtering instead to a positive active count reads more honestly — no row
without a bar — but its height tracks how much work happens to be in flight, so
the card breathes in and out and empties completely on a quiet day. A fixed cap
holds the band still.

**Rejected: show only repositories with active changes.** Cleaner rows, unstable
height, and an empty card on the very days the Dashboard is meant to be
encouraging.

**Rejected: show every active repository, backfilled to five.** Never hides work
in flight, but reintroduces the variable height for exactly the registries —
many repositories, much in flight — where a bounded card matters most.

### Two row shapes, so the bar never contradicts the sort

A list sorted by one quantity and drawn with bars encoding another reads as a
broken sort. Since the sort key is active changes, the bar must encode active
changes — which leaves rows at zero with an empty track. Five empty rectangles
in a column is the defect the change set out to remove.

So rows take one of two shapes. A row with work in flight draws a bar; a row
without draws none and dims to a label and an archived count. The break between
the groups is legible without a heading, and every bar on screen has a length
worth comparing.

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 270" width="640" height="270">
  <rect x="0" y="0" width="640" height="270" fill="#0e1218"/>
  <line x1="14" y1="24" x2="626" y2="24" stroke="#2b323d"/>
  <text x="14" y="18" fill="#8e9aab" font-family="monospace" font-size="9">OVERVIEW</text>
  <text x="626" y="18" fill="#8e9aab" font-family="monospace" font-size="9" text-anchor="end">39 archived · 14 days · avg time-to-archive 1.1d</text>
  <rect x="14" y="38" width="612" height="214" rx="6" fill="#13171e" stroke="#2b323d"/>
  <text x="30" y="60" fill="#8e9aab" font-family="monospace" font-size="9">PER REPOSITORY</text>
  <text x="30" y="86" fill="#f1f4f8" font-family="sans-serif" font-size="11">Meter Burn</text>
  <rect x="150" y="78" width="220" height="6" rx="3" fill="#232a36"/>
  <rect x="150" y="78" width="220" height="6" rx="3" fill="#4f5bd9"/>
  <text x="610" y="86" fill="#8e9aab" font-family="monospace" font-size="9" text-anchor="end">2 active · 57 archived</text>
  <text x="30" y="112" fill="#f1f4f8" font-family="sans-serif" font-size="11">TouchPoint</text>
  <rect x="150" y="104" width="220" height="6" rx="3" fill="#232a36"/>
  <rect x="150" y="104" width="110" height="6" rx="3" fill="#4f5bd9"/>
  <text x="610" y="112" fill="#8e9aab" font-family="monospace" font-size="9" text-anchor="end">1 active · 2 archived</text>
  <text x="30" y="138" fill="#f1f4f8" font-family="sans-serif" font-size="11">Pannonfox</text>
  <rect x="150" y="130" width="220" height="6" rx="3" fill="#232a36"/>
  <rect x="150" y="130" width="110" height="6" rx="3" fill="#4f5bd9"/>
  <text x="610" y="138" fill="#8e9aab" font-family="monospace" font-size="9" text-anchor="end">1 active · 0 archived</text>
  <text x="30" y="176" fill="#5c6675" font-family="sans-serif" font-size="11">MushRoom</text>
  <text x="610" y="176" fill="#5c6675" font-family="monospace" font-size="9" text-anchor="end">276 archived</text>
  <text x="30" y="200" fill="#5c6675" font-family="sans-serif" font-size="11">SpecForge</text>
  <text x="610" y="200" fill="#5c6675" font-family="monospace" font-size="9" text-anchor="end">111 archived</text>
  <line x1="30" y1="218" x2="610" y2="218" stroke="#2b323d"/>
  <text x="30" y="238" fill="#5c6675" font-family="monospace" font-size="9">+ 9 more · none active</text>
</svg>
```

**Rejected: a stacked bar encoding active and archived together.** It would
finally visualise the archived volumes, which are the largest numbers on the
card — and it would put a four-pixel segment first and a full-width bar fourth,
which is precisely the broken-sort reading.

**Rejected: dots instead of bars.** Honest at counts of one and two, where a
proportional bar overstates, but it imposes a ceiling and degrades exactly when
a repository gets busy.

### The bar's length encodes a count, not the pane width

Bars are normalised against the largest active count on the card and clamped to
a maximum:

$$w_i = w_{\max}\cdot\frac{a_i}{\max_j a_j}, \qquad w_{\max} = 220\ \text{px}$$

The existing rule makes the track `flex: 1` inside a half-width card. Promoting
the card to full width without changing that turns six hundred pixels of accent
into the number two — the chart's proportion problem, transposed onto the card
that replaced it. The card still fills the pane, as the *Dashboard Fills
Available Width* requirement demands; the mark inside it does not, because its
length is data.

**Rejected: keep the track proportional and let it follow the pane.** Satisfies
the width requirement most literally and produces the worst drawing at the
widths the requirement exists to serve.

### Order in the payload, cap in the frontend

The two halves of the presentation split differently, because only one of them
changes what the array totals to.

Ordering goes in `repo_breakdowns`. A permutation is invisible to any sum, so
sorting there costs the payload's consumers nothing, and it puts the comparator
— three keys, one of them purely a stability tie-break — where `cargo test`
reaches it and where a dropped key fails a test rather than being noticed on
screen weeks later.

Capping stays in `DashboardView`, which computes the page's closing footnote by
summing the archived count across the whole breakdown array. Capping that array
in `repo_breakdowns` would silently reduce the registry-wide total to the sum of
five rows, in a footnote sitting a few hundred pixels below the card that caused
it. The payload keeps every entry; the frontend shows the first $N$ of them.

**Rejected: cap in `repo_breakdowns` and send a separate archived total.** Two
fields that must agree, to avoid one array the frontend can already slice.

**Rejected: sort in the frontend too, for symmetry.** It puts a three-key
comparator in the layer with no test that can see it, and asks every future
frontend to re-derive an order the payload could simply have arrived in.

### The remainder line reports what the cap hid, not what it kept

"+ 9 more · none active" rather than "+ 9 more · 765 archived". The
registry-wide archived total already appears in the footnote below, so repeating
it says nothing new; what a truncated list owes its reader is whether anything
*in flight* was withheld. When something was, the line says so — "+ 9 more · 3
active" — and the omission is legible rather than silent.

### Ships is promoted but keeps its quiet-day note

Moving the feed above the heatmap places it directly beneath the haul tiles,
whose zero state already reads *"A fresh day — check off a task or land a commit
to get the ball rolling."* On a morning before the first ship, two consecutive
empty-state sentences appear. That is accepted: the alternative — hiding the
feed when empty — makes the section order itself depend on the time of day, so
the heatmap moves up and down the page as the morning progresses. A stable
layout is worth one redundant sentence, and the *Today's Ships Quiet State*
requirement already forbids hiding the feed.

**Rejected: suppress the note whenever the haul's nudge is showing.** Couples
two independent components through a condition neither owns, to remove a
sentence that is only ever visible for part of one day.

### Lifecycle figures move onto the band's divider rule

The two figures lose their host when the chart goes. Placing them on the
`OVERVIEW` rule — band label left, figures right — adds no box to a page already
stacking six, and puts the band's summary where a band's summary belongs.

They also lose their referent: "39 archived this window" was legible only beside
a card captioned "last 14 days". The window is now named in the figures
themselves.

**Rejected: a slim strip card above the breakdown.** Most prominent, and a
seventh box on the page for one line of text.

## Risks / Trade-offs

- **The only all-author commit signal is removed.** On a shared repository the
  band no longer shows the team's daily commit volume. *Mitigation:* the
  leaderboard retains per-author commit totals over a year and the commit garden
  retains today's per-author, per-repository detail; the gap is confined to
  daily granularity over a fortnight. Accepted deliberately, and recorded here so
  a future team view starts from a decision rather than from an omission.

- **A registry with more than five active repositories hides work in flight.**
  *Mitigation:* the remainder line states how many withheld entries carry active
  changes, so the reader knows to look; the tree pane remains the complete view.

- **Removing a requirement is heavier than removing a card.** *Git-Mined
  Activity Chart* is referenced by three other requirements, and its window
  definition is relied on by a fourth that never restates it. *Mitigation:* all
  four are modified in the same delta — *Change Lifecycle Metrics* absorbs the
  window definition, and *Graceful Degradation Without Git* and *Dashboard
  Includes Disabled Workspaces* drop the chart from the surfaces they enumerate.
  The capability's `## Purpose` paragraph names the chart too and is corrected at
  sync.

- **A dropped tie-break is invisible to the tooling that would normally catch
  it.** The comparator sits in a mutation-gated crate, but `cargo mutants`
  generates whole-function replacements — for `repo_breakdowns` it emits
  `vec![]` and `vec![Default::default()]`, not a comparator with its third key
  removed. So the gate confirms the function does something; it cannot tell
  whether the function orders correctly. Nor can a test that sorts a fixture
  containing no ties: dropping the archived key or the label key reorders
  nothing there, and the symptom in production — rows swapping places between
  refreshes — never appears in a single assertion.
  *Mitigation:* the fixture ties deliberately, once on the active count and
  again on the archived count, and the assertion pins the full resulting order
  rather than membership. That test, not the mutation gate, is what defends the
  ordering. The cap is frontend-side and outside the gate entirely, covered by
  an ordinary test at $N$ and $N+1$ entries.

- **A registry smaller than the cap does not fill the card.** With two
  repositories registered the card shows two rows and the constant height the
  change is for does not materialise. *Mitigation:* accepted — a registry that
  small has no dead space to remove, and padding it with blank rows would be
  furniture.
