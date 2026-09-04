# Design

## Context

The reading column is not one number. `visual-identity`'s *Markdown Body Adopts
the Type System* specifies a **two-tier** column and the stylesheet implements it
in four rules:

```
.markdown-view            max-width: 880px    object tier — tables, pre, mermaid,
                                              svg, katex-display
.detail-identity-inner    max-width: 880px    the identity header, aligned to the
                                              same column by hand
.markdown-view p          max-width: 74ch     prose tier
.markdown-view li / blockquote
  :not(:has(pre, .table-scroll, .mermaid-block,
            .svg-block, .katex-display))
                          max-width: 74ch     prose tier, lifted wherever the
                                              block contains an object
```

Three facts about that arrangement shape every decision below.

**The tiers are not proportional by accident.** 880px of 16px Inter runs ~110
characters (~114 measured); the 74ch measure cuts that to ~97 — the stylesheet's
own comment says ~78, and is wrong, for reasons taken up under the rungs below.
So prose occupies roughly 85% of the column, and the gap is what lets a table be
wide while a paragraph is not. Any control that moves one tier without the other
either strands prose in a column it no longer relates to, or narrows the objects
the column exists to serve.

**The `ch` unit and the `px` cap are measured against different things.** The
measure is `ch` of the prose font and the column is absolute, so they are locked
together only by the numbers chosen. This is a feature — a future type-scale
setting would move the measure and leave the column alone, which is correct — but
it means the ladder's rungs are two independent series that must be picked as
pairs, not derived from one scale factor.

**The column is already smaller than the space available.** `SplitPane` is
drag-resizable and both side panes collapse, so the detail pane routinely exceeds
the column by a wide margin, while a reader window at its default 720×820 offers
656px of content and is narrower than either tier. The fixed numbers are therefore
inoperative at one end and leaving space unused at the other.

A fourth fact governs the delivery mechanism rather than the geometry:
`src/main.tsx` stamps `data-platform` and `data-surface` on `document.body`
**before** `createRoot`, and says why — "so CSS that keys off it … is in effect
from the first paint". A preference living behind an async IPC call cannot use
that door unmodified.

## Goals / Non-Goals

**Goals**

- Let the reader choose a reading width in both directions from today's, as a
  preference rather than as a fix for a specific complaint.
- Keep the two-tier relationship intact at every rung, so no setting produces
  prose stranded in a column or objects paying for the measure.
- Make the widest rung actually solve the wide-diagram case that `figureFloor`
  currently mitigates by scrolling.
- Have the chosen width in effect on the first frame, on every surface, in both
  hosts.
- Retire the duplicated `880px` literal as a consequence of the mechanism rather
  than as a separate tidy-up.

**Non-Goals**

- A font-size or zoom control. The measure is in `ch`, so type scale is a
  separate axis and stays out of this change.
- Per-document or per-window width. One global value.
- Restructuring the two-tier model, the `:has()` guard, or the headings exemption.
- Giving the terminal frontend a prose measure.
- Continuous adjustment. See the first decision.

## Decisions

### Presets, not a slider

A continuous pixel slider is the obvious shape and the wrong one here, because the
thing being adjusted is a *pair* of values with a relationship. A slider either
exposes one number and breaks the pair, or exposes two and asks the reader to
maintain a typographic invariant by hand.

Presets also keep the contract statable. A requirement can say "at every rung
prose is bounded and the object tier is at least as wide as the prose tier"; the
equivalent for a slider is a formula that has to be restated in the spec, the
stylesheet, and the test.

And they keep the ladder testable as data: `src/docWidth.ts` is a table plus a
fallback, which is a pure function of its input.

**Rejected: a slider with a coupling curve** — prose derived from the column by a
fixed ratio. It removes the pairing problem but replaces four legible rungs with a
curve nobody can predict from the UI, and makes "what does Wide mean" unanswerable
in a spec.

**Rejected: a per-document live control** (`⌘+`/`⌘−` on the reading surface, like
`figureZoom`). Cheapest to build and the most immediate to use, but it forgets
between sessions, and a reading width the reader has to re-establish on every
document is worse than one they cannot change.

### Both tiers move, and the sanctioned band widens deliberately

The current requirement bounds prose to 70–80ch. Holding that band across the
ladder would make every rung differ by less than 15% in prose and up to 45% in
column — the objects would move and the text would barely budge, which reads as a
broken control.

So the band widens to 50–96ch. It widens in both directions, and the two ends are
not the same kind of concession. `50ch` moves prose *into* the comfortable range,
which the 70–80ch band never actually delivered (see below); `96ch` moves past it
deliberately, at the reader's request, and is bounded so it cannot run away. The
alternative is a preference that does not visibly prefer anything.

**Rejected: fix the measure and move only the column.** Attractive because it
preserves the typographic guarantee exactly, and because prose would still narrow
at the low end for free (a prose block cannot exceed its ancestor's content box,
so a 720px column clamps a 74ch measure without being asked). But at the wide end
`Wide` and `Full` would then change nothing for a document that is mostly prose —
which is most documents in an OpenSpec workspace.

### The rungs

| Preset | `--doc-measure` | px | `--doc-column` | Prose chars | Code chars |
|---|---|---|---|---|---|
| `compact` | `50ch` | 505 | `720px` | ~65 | ~76 |
| `default` | `74ch` | 747 | `880px` | ~97 | ~94 |
| `wide` | `86ch` | 868 | `1040px` | ~113 | ~112 |
| `full` | `96ch` | 969 | `none` | ~125 | pane |

Every figure is measured off rendered line boxes, not derived. That distinction
turned out to matter — see the next decision.

The column steps by a constant 160px. The measure deliberately does not: the
prose-to-column ratio is ~85% at `default` and `wide`, and 70% at `compact`.
`compact` is the rung a reader reaches for *because the text feels wide*, so it
tightens the text more than the container. Stepping evenly (`62ch`) would have
put it at ~83 characters — a narrowing, but not into the range anyone calls
comfortable, so the rung would have looked like it worked while not doing the
one thing it exists for. At `50ch` it lands at ~65.

`default` is today's rendering unchanged, so no existing install moves.

### The `ch` unit buys more characters than it says, and the old numbers were wrong

The first version of this ladder was chosen from arithmetic in the existing
stylesheet comment: that 880px runs ~110 characters, and that the 74ch measure
cuts that to ~78. The first figure is about right (~114 measured). The second is
not — 74ch renders **~97** characters.

The unit is why. `ch` is defined as the advance of the digit zero, which in Inter
Variable at `--text-lg` is 10.09px, while an average prose character is ~7.6px.
A `ch` measure therefore buys about 1.33× the characters its number suggests, and
any figure derived by treating `ch` as "characters" is out by a quarter.

Two consequences, both settled deliberately rather than absorbed:

**The rungs were retuned.** Under the corrected numbers the original ladder
spanned ~83 to ~125 characters — entirely above the comfortable range, with no
rung serving a reader who wants a tighter measure, which was half the stated
motivation. `compact` moved from `62ch` to `50ch`.

**The default's own line length is above the range the spec recommends** — ~97
characters, against the 60–90 that same requirement cites — and it is left
exactly as it is. That rendering is what every existing install already has;
changing it would be a visual-identity decision, not a side effect of adding a
preference, and this change explicitly promises the default rung does not move.
What changes is the *claim*: the requirement and the `App.css` comment are
corrected to state the measured figure and to warn against re-deriving it from
the `ch` value. A reader who wants the comfortable measure now has `compact`.

**Rejected: retune the default to land inside the range.** It would make the
spec's own recommendation true, and it is the change most likely to be wanted
eventually. But it silently re-wraps every document in every existing install
under cover of a change whose headline promise is that the default is untouched.
It belongs to a `visual-identity` change that argues for it on its own terms.

### `Full` fills the pane, and prose is still bounded

`full` sets `--doc-column: none`, so objects take whatever the pane gives them.
This is the rung that pays for itself: `figureFloor.ts` documents that a ~2580px
flowchart fitted into 880px scales to 0.34 and renders 15px labels at ~5px, at
which point the floor stops the shrink and the diagram scrolls inside its block.
On a 2000px pane at `full`, that diagram renders at 0.78 — legible, unscrolled,
and no longer fighting a cap that existed for prose.

Prose does **not** become unbounded with it. `--doc-measure` stays at `96ch`, so
the widest rung still refuses the ~200-character line an unbounded column would
produce on a 4K display. `full` is "objects fill, prose is capped", not "no
column".

**Rejected: a bounded `widest` rung (~1200px)** instead of fill. It keeps every
rung a centred, visually coherent column and avoids the mismatch below — but 1200px
still scales the same flowchart to 0.47, so the case that motivated the wide end
of the ladder would remain unsolved by the widest rung.

### Headings span the pane at `Full`, and that is accepted

Headings are deliberately exempt from the measure so the hairline rules beneath
`h1`/`h2` span the reading surface and read as section boundaries. At `full` on a
2500px pane that is a 2500px rule above an 810px paragraph.

That is accepted rather than special-cased. The rule's job is to bound the reading
surface, and at `full` the reading surface *is* the pane — a rule that stopped
short of the tables beneath it would be describing a column that no longer exists.
Capping heading rules only at `full` would also make the headings exemption
conditional, which is exactly the kind of rule-with-an-exception the two-tier
model was written to avoid.

Recorded here so that if it does read badly in use, the next change starts from a
decision rather than from an oversight.

### `Full` is not the widest rung on a narrow pane

In a default 720×820 reader window, `full` resolves to 656px of content — narrower
than `default`'s 880px. The ladder is an ordering of *intent*, not of resulting
pixels, and `full` is the only rung whose result depends on the surface.

No mitigation is proposed, because the behaviour is correct: the reader asked for
"use the space available" and there is less of it. It is stated in the spec so
that a reader window rendering narrower after choosing the widest preset is
documented behaviour rather than a bug report.

### Custom properties on a body attribute, with explicit fallbacks

```css
:root                          { --doc-column: 880px;  --doc-measure: 74ch; }
body[data-doc-width="compact"] { --doc-column: 720px;  --doc-measure: 50ch; }
body[data-doc-width="wide"]    { --doc-column: 1040px; --doc-measure: 86ch; }
body[data-doc-width="full"]    { --doc-column: none;   --doc-measure: 96ch; }
```

A body attribute rather than inline styles on the view: it matches the existing
`body[data-platform="mac"]` / `body[data-surface="reader"]` pattern, it can be
stamped before React mounts, and it reaches all four reading surfaces plus the
Settings sample through one write. `:root` carries the default rung so an
unstamped body — a surface that somehow renders before the stamp — is today's
rendering rather than nothing.

**Every consuming declaration carries a fallback**: `var(--doc-column, 880px)`,
`var(--doc-measure, 74ch)`. This is not defensive habit. An unresolvable custom
property is invalid at computed-value time, and `max-width`'s unset value is
`none` — so a typo in a property name would silently remove the prose measure and
the column cap and produce full-bleed text with no error anywhere. The fallback
turns that failure into "renders at the default rung".

One constraint the token form imposes: `--doc-column` can hold the keyword `none`,
so nothing may do arithmetic on it. `calc(var(--doc-column) / 2)` would be invalid
at `full`. No current rule needs to; a future one must derive from the numeric
rungs instead.

### `localStorage` mirrors the preference so the first paint is correct

`AppSettings` remains the source of truth. On every write the preset is also
written to `localStorage`, and `main.tsx` stamps `data-doc-width` from that mirror
synchronously before `createRoot`. Once the authoritative value arrives from
`get_document_width`, the stamp is reconciled — which is a no-op in the ordinary
case and corrects the mirror when another instance changed the setting.

This is two stores for one value, which is a real cost, and it is paid for a
specific reason: `main.tsx` already establishes that surfaces keyed off a body
attribute must have it from the first frame, and a width flash is the most visible
kind — the entire document reflows.

Reader windows share the Tauri webview origin, so a reader opened after a change
reads the correct mirror without any event at all. The event below exists only for
readers that were *already* open.

**Rejected: accept the flash.** One IPC round-trip, no second store, no
reconciliation — and every cold load reflows the whole document once. For a
setting whose entire purpose is how the document is laid out, that is the worst
place to spend a frame.

**Rejected: gate the first render on the fetch.** No flash and no second store,
but it puts a loading state in front of a surface that paints immediately today,
and it would have to be added to two entry points (`App` and `ReaderRoot`).

### A dedicated event, not a general `settings-changed`

`document-width-changed` carries the new preset and is emitted directly by the
setter in both transports, following `EVENT_WORKSPACE_PRESENTATION_UPDATED`'s
precedent exactly: not a `CacheEvent` variant, because — as `events.rs` records
for `document-changed` — expressing it as one "would have forced every existing
consumer of that stream, in three frontends, to grow an arm that ignores it".

The SSE bridge already carries generic `(String, Value)` frames, so the web
transport needs no new machinery; the Tauri side is an `emit` at the command.

**Rejected: a general `settings-changed` event.** More reusable, and the shape
this will eventually want if a second live-propagating preference appears. But it
forces every consumer to re-fetch and diff to discover whether the thing it cares
about moved, and there is exactly one such preference today. A general event is
the right second step, not the right first one.

### The Settings section carries a live sample

Settings is a routed view (`address.kind === "settings"`) that replaces the
document rather than overlaying it, so a reader picking a rung cannot see the
effect on anything they were reading. The section therefore renders its own
sample — one paragraph and one narrow code well — inside a container carrying the
selected rung's properties.

The sample is scoped to that container rather than to `body`, so it previews a
rung without committing it; the body attribute is written only when a rung is
chosen.

### The terminal frontend is excluded, deliberately

`ui.rs` renders the detail pane with `Wrap { trim: false }` at full pane width, so
the TUI has no measure of any kind and prose runs to the terminal's width. Giving
it one is a reasonable change; it is not this one, and it would need its own
answer for what a `ch`-based measure means in cells.

The consequence is that `specforge-tui` reads a settings file carrying a
`documentWidth` field it ignores. That is stated in the spec so a future reader
finds a decision rather than an omission.

## Risks / Trade-offs

- **The sanctioned band widens to 50–96ch, and 96ch is past comfortable
  reading.** *Mitigation:* the default is unchanged, so the requirement still
  describes what every install renders until someone deliberately chooses
  otherwise; the band remains bounded at both ends, so no rung produces the
  unbounded line an uncapped column would; and the requirement states the default
  rung and the ladder's limits separately, so the two claims cannot be confused.
  Note the band widens *downward* too — the guarantee is not only weakened, since
  no previous rung reached the comfortable range at all.

- **The default's line length is knowingly left outside the range its own
  requirement recommends.** ~97 characters, against the 60–90 cited a sentence
  earlier in the same requirement. *Mitigation:* stated plainly in the
  requirement and in `App.css` rather than papered over, with the measured figure
  and the reason the `ch` unit misleads; and `compact` gives a reader who wants a
  comfortable measure one, which is the practical remedy. Retuning the default is
  left to a change that argues for it — see the decision above. The risk being
  accepted is that a reader of the spec sees a recommendation the default does
  not meet; the risk being refused is silently re-wrapping every document in
  every install.

- **Two stores for one value can disagree.** A second instance — another
  worktree's dev build, the web UI in a browser, `specforge-serve` — writes
  `AppSettings` and this instance's `localStorage` mirror goes stale.
  *Mitigation:* the mirror is only ever a first-paint hint; the authoritative
  fetch on mount reconciles it, and the reconciliation is the same code path as
  the initial stamp. The worst observable outcome is one reflow shortly after
  launch, in the case where the setting changed elsewhere since this window last
  ran — which is exactly the case where a reflow is correct.

- **A missing custom property fails silently and totally.** `max-width` with an
  unresolvable `var()` computes to `none`, so a renamed or misspelt property does
  not degrade — it removes both the measure and the column at once, on every
  reading surface. *Mitigation:* every consuming declaration carries an explicit
  fallback to the default rung's value, so the failure mode is "renders like
  today". This is the one place in the change where the defensive form is
  load-bearing rather than habitual, and it is commented as such.

- **`Full` looks wrong at extreme widths.** An 810px paragraph beside a 2400px
  table, under a 2400px heading rule, is visibly two different documents.
  *Mitigation:* none, and accepted — `full` is opt-in, its purpose is the objects,
  and the alternative (a bounded widest rung) fails the diagram case that
  motivated it. The heading decision above records the same trade-off for the
  rules specifically.

- **The frontend carries most of the logic and the least gating.** `src/` is
  outside the mutation gate entirely, and a diff touching only `src/` and
  `crates/specforge*` short-circuits the Mutants job — it will report green in
  seconds without running. *Mitigation:* `src/docWidth.ts` is pure and unit
  tested (preset table, unknown-value fallback, mirror read/write), following the
  precedent `figureFloor.ts` and `figureZoom.ts` set for exactly this reason. The
  green Mutants report on this change means "not run", not "covered".

- **The gated crate's share is small and needs its assertion written by hand.**
  `settings.rs` is in `openspec-app` and *is* gated, but the new surface there is
  an enum with `#[serde(other)]` — and `cargo mutants` generates whole-function
  replacements, so it will not produce a mutant that removes a serde attribute.
  *Mitigation:* an explicit test that a settings file containing an unrecognised
  `documentWidth` loads as `Default` **with every other field intact** — the
  failure being guarded against is a strict enum turning one unknown string into a
  total settings-load failure, which would lose the workspace registry.

- **CSS geometry has no automated coverage in this repository.** Nothing asserts
  the computed `max-width` of a paragraph today, and this change adds four rungs
  of it. *Mitigation:* the preset table is tested as data, and the DOM assertion
  is made once, manually, through the `specforge-web` verification route against a
  document containing a paragraph, a wide table and a mermaid diagram at each
  rung. The tasks state this explicitly rather than leaving verification to
  whoever implements it.
