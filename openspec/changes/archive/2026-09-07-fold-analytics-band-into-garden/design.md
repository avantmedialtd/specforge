# Design

## The question this change answers

The band's rework two days ago optimised a presentation. This change asks the
prior question — does the surface earn its place — and the answer turns on one
audit: what does the band tell the reader that the Dashboard does not already
tell them elsewhere?

```
 BAND DATUM                    UNIQUE?   ANSWERED ELSEWHERE BY
 ─────────────────────────────────────────────────────────────────────────
 per-repo active count           no      footnote's registry-wide total
                                         (this is its decomposition);
                                         tree pane, per row, uncapped
 per-repo archived count         no      footnote's registry-wide total
 per-repo label                  no      garden plot caption; tree pane
 proportional bar                —       encodes the active count above
 cap · remainder · tracks        —       chrome supporting the decomposition
 archivedInWindow · window ·    YES      nothing
   avgTimeToArchive
```

Two rows of unique content out of seven, and both are fourteen-day trend
aggregates on a surface whose every other element answers *today* or *right now*.
The rest is a decomposition, and a decomposition capped at five entries is a
worse answer to "which repository holds my work" than the uncapped tree pane
standing to the left of it.

## Why the garden is the survivor rather than the destination

An earlier framing of this change was a *merge*: keep both surfaces and join
them into one per-repository list, each row carrying its counts and its plot.
That framing was abandoned, and the reasons are worth recording because they
explain why the final shape is so much smaller.

**The two surfaces arrive on different fetches.** The breakdown rides
`getDashboard`; the garden rides `getCommitGarden`, which additionally refreshes
on a local-midnight tick and on window focus (`useCommitGarden.ts`). A merged row
needs both, which forces a law: the garden may change a row's *content* but never
the list's *order* or *membership* — otherwise the list visibly re-sorts, or rows
pop in and out, every time the window regains focus. That law then made the merge
awkward in a specific way: a repository with commits today but no OpenSpec changes
sorts last by the breakdown's keys and falls off the five-row cap, so its commits
would vanish from a surface that renders them today.

**The join key was unsound.** `RepoBreakdown` and `WorkspaceGarden` both carry
only `label`, computed identically as `display_name.unwrap_or(name)`
(`dashboard.rs:458`, `service.rs:1095`). It is a presentation string with no
uniqueness guarantee, so two entries sharing a display name would attach the
wrong plot to the wrong row.

Deleting the breakdown dissolves both problems rather than solving them. There is
no cross-fetch reflow to reason about because there is no second list, and there
is no join because the one surviving figure travels on the plant itself.

## Carrying the active count without a join

`WorkspaceGarden` gains an `active_count`, filled in `service.rs` at the point
that already fills `plant.label`, from the `WorkspaceView` in hand —
`r.active.len()` for a repository group, `changes.len()` for a flat workspace.
One field, one assignment, no lookup, no key.

Flat workspaces are always dormant and so never rendered, but the field is
populated for them anyway rather than left at zero: a dormant plant's other
fields are already honest, and a zero that means "not computed" is the kind of
value that later reads as data.

## Scope of the active count

The caption's count is **registry-wide per entry**, exactly as the breakdown's
was. It is therefore not comparable with the hero's `in flight` tile, which is
developer-scoped (`scoped_in_flight`, `service.rs:1506`, per the *Personal
Progress Frame* requirement). Two different figures share the word "active".

This asymmetry is not introduced here — it exists today between the same tile and
the breakdown row directly below it — and correcting it is out of scope. The
Dashboard is deliberately "the unfiltered record of what the user has registered
and accomplished" (*Dashboard Includes Disabled Workspaces*), so a registry-wide
per-entry count is the right figure; it is the tile that is narrowed, by an
explicit requirement, and the tile is not what this change touches.

## What survives the deletion, and why it is not obvious

Two pieces of the removed surface stay alive, and both are easy to remove by
mistake.

**The lifecycle mining subsystem.** `assemble` mines each repository's lifecycles
once and spends them twice (`dashboard.rs:183-187`): on the metrics, and on
dating each entry in today's ships feed. The metrics are what this change
deletes. The mining, its per-repository invalidation, its collapsing of
concurrent derivations and its distinction between "no changes" and "mining
failed" all remain load-bearing for the ships feed's `archived <time>` stamps.
The requirement is renamed rather than modified — *Change Lifecycle Mining*
rather than *Change Lifecycle Metrics* — because a requirement named for metrics
it no longer specifies is a trap for the next reader.

**`repo_breakdowns` as pure data.** The footnote's registry-wide archived total
is a reduction over the vector (`DashboardView.tsx:511`). The removed requirement
already anticipated this case in as many words: withholding is presentational,
and the underlying data retains every top-level item. That clause is the only
part of *Per-Repository Breakdown* worth keeping, so it moves to
*Cross-Workspace Summary Metrics*, which is the requirement that consumes it.

The vector's **sort**, however, does not survive. Its three keys existed so that
rendered rows would not trade places between refreshes; with no rows, the order
is unobservable. Keeping it would leave a comparator whose only remaining
justification is the tests asserting it — and a passing mutation gate on a
comparator nothing orders by is not coverage of anything.

## Why the garden needs an order it never had

The garden maps `views` straight through, so its plots are in registry order with
no tiebreak at all. Two entries can trade places between refreshes, which is
precisely the defect *Per-Repository Breakdown*'s three-key sort was written to
prevent — and the garden has been sitting directly beneath that well-ordered list
the whole time, which is why nobody has had cause to notice.

Removing the breakdown promotes the garden to the Dashboard's only
per-repository list. Shipping that promotion without an order would be shipping a
regression, so the order is in scope: **today's commit count descending, then
label ascending.** Commits first because the section is about today's activity
and the busiest repository is the one worth leading with; label as the tiebreak
because it is total and stable, so two equally busy repositories hold their
position across refreshes.

Note this is *not* the breakdown's ordering carried over. The breakdown ranked by
work in flight because that is what it was about; the garden ranks by commits
because that is what *it* is about. The active count rides along in the caption
as an annotation and deliberately does not participate in the sort — sorting by
it would reintroduce the two-scope confusion the section otherwise avoids.

## Alternatives considered

**Keep the lifecycle figures somewhere unconditional.** The garden omits itself
entirely on a quiet day (*Dormant and Degraded States*), so figures hung on its
heading vanish with it — even though a fourteen-day window has nothing to do with
whether the viewer committed today. The footnote and the hero were both viable
homes. Rejected in favour of deleting the figures: relocating them would have
preserved, at the cost of new placement, three numbers that no requirement
depends on and that no user action follows from. If the trend question returns it
deserves a surface designed for it, not a corner of a footnote.

**Keep the breakdown, drop only the lifecycle figures.** This leaves a band whose
entire content is a capped decomposition of the footnote directly below it, under
two headings. It is the smaller change and the worse outcome.

**Merge the two surfaces row-by-row.** Covered above: forced by the fetch split
into a design where the cap could hide today's commits, and dependent on an
unsound label join.

## Risks

**The mutation gate.** `openspec-core` and `openspec-app` are the gated crates
and this change touches both. Deletions are safe — a removed function's mutants
go with it — but the two *additions* are exactly the shape the gate exists to
catch: a new comparator (the plot order) and a new payload field
(`active_count`). Both need assertions that fail when broken, and per this
repository's own experience a green gate on a comparator is not evidence of
ordering coverage. The plot order needs an adversarial fixture with a genuine
tie, asserting the label tiebreak resolves it, not merely that sorted output
comes back sorted.

**Silent frontend breakage.** The debug `specforge-web` build reads `dist/` from
disk per request, so a stale bundle will happily serve the pre-change Dashboard
during verification. `bun run build` before trusting any visual check.

**Spec-text drift.** Six requirements outside the two being removed refer to the
band, the breakdown or the metrics by name, including two cross-references buried
in unrelated requirements (*Personal Progress Frame*'s carve-out for repository
ordering, and the heatmap's day-breakdown scenario contrasting itself with "the
band's live in-flight count"). Missing one leaves the synced spec naming a
requirement that no longer exists.
