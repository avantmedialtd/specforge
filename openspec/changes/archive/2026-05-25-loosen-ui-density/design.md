## Context

The current `--text-*`, `--leading-*`, and tree-row padding values in `src/App.css` were tuned on a 2× Retina display. At 4K @ 100% OS scale — the developer's primary workstation — every CSS px is one device px, so the sidebar tree (13px label, 1.4 line-height, 2px vertical row padding) renders at roughly 22px row height with 9–10pt-equivalent labels. The user reports the sidebar feels cramped; detail pane and settings are tighter than desired but not the main offender.

The exact px values for the type scale and the markdown body are pinned in `openspec/specs/visual-identity/spec.md` (the scale is part of the visual contract, not just a CSS detail). Any retune is therefore a spec change, not just a stylesheet edit.

## Goals / Non-Goals

**Goals:**

- Make the sidebar tree comfortably scannable on a 4K display at 100% OS scale, without leaning toward the screen.
- Apply a single, consistent bump across the whole token scale rather than spot-fixing only the sidebar — keeps the visual hierarchy intact and avoids future drift between sidebar and detail-pane proportions.
- Preserve the dense-browser identity of the sidebar: the tree should still show roughly a full workspace's worth of rows at a typical sidebar width.
- Keep the token *names* and *spacing scale* (`--space-1` … `--space-7`) unchanged so existing call sites carry over without renaming.

**Non-Goals:**

- A user-facing density toggle (compact/comfortable). We're committing to a single comfortable default for 4K @ 100% and will revisit only if real-world feedback says otherwise. A toggle would double the design surface and force every future component to be specified at two densities.
- Changes to font family, font weight, color palette, chip/outline conventions, or dark-scheme overrides.
- Re-tuning the space scale itself. Only token *values* change; the 4/8/12/16/24/32/48 progression stays.
- `--leading-prose` (1.65) and detail-pane padding. These were already tuned for reading and stay as-is. (The markdown `max-width` does move — see Decisions below — but only as a direct consequence of the body font growing from 15 → 17px, not as a separate redesign of the reader.)

## Decisions

### Uniform +2px bump across the type scale

```
token          before  after  delta  used by
─────────────────────────────────────────────────────────────────
--text-xs        10      12     +2   chips, count, mono badges
--text-sm        11      13     +2   row meta, settings help
--text-base      12      14     +2   settings body, empty state body
--text-md        13      15     +2   tree label, default body, h4
--text-lg        15      16     +1   markdown body, h3
--text-xl        20      22     +2   markdown h2, settings h1
--text-2xl       28      30     +2   markdown h1
```

Most tokens take a uniform +2px shift; `--text-lg` takes only +1 because it drives the markdown body and a +2 (to 17) made the reader column feel too head-heavy relative to its 15px fenced-code blocks. Keeping `--text-lg` at 16 preserves a one-step gap from `--text-md` (15) and brings the body visually closer to inline/block code so prose and code feel like parts of the same composition rather than text dominating code.

Considered alternatives:

- *Uniform +2 across the whole scale (lg → 17)* — was the original plan and is the cleaner rule, but the markdown reader needed to be re-tuned in light of how it renders both prose and code, and 17px body next to 15px code felt unbalanced.
- *Bump only `--text-md` and `--text-sm`* — would make the sidebar feel right but desync proportions between sidebar meta and detail-pane body; future components would inherit the inconsistency.
- *Larger bump (+3–4px) across the board* — tested mentally against the 32" 4K case and felt closer to a tablet/Notion default than to the dense-browser identity we want.

### Line-height: `--leading-tight` 1.4 → 1.5

The 1.4 default was acceptable at 13px but feels stacked when labels grow to 15px (the descender/ascender ratio of Inter benefits from a touch more leading). `--leading-prose` (1.65) is already well-tuned for reading and stays; `--leading-code` (1.5) stays because monospace already has even vertical metrics.

### Tree-row vertical padding: 2px → 5px

The sidebar is where the cramping is most acute. Padding has more visual leverage than font size alone — going from 2px to 5px takes `.tree-row` from ~22px to ~30px height (with the 15px label and 1.5 line-height), which feels like a Linear-style list rather than a compressed Finder column. The horizontal padding (`8px` right, `4px` left for the selection bar) is unchanged.

Alternative considered: bump font but keep 2px padding. Rejected — labels would touch top/bottom edges of the row's hit area, and the hover background would feel like it's pasted onto the label rather than framing it.

### Workspace-row padding: `--space-3` → `--space-4` (12 → 16)

`.workspace-row` already had reasonable proportions; the bump is just to track the type scale change so the settings list doesn't end up tighter-feeling than the new sidebar.

### `.settings-toggle-row` padding: 4 → 6

Tiny bump so the checkbox row in settings doesn't feel comparatively cramped next to the workspace rows above it.

### Markdown view `max-width`: 760 → 880

Two forces motivate the wider column. First, the body font growth (15 → 16) trims a few characters per line if the container stays at 760, while the heading scale (h1 30px, h2 22px) needs more horizontal canvas to feel proportionate on a 4K detail pane. Second — and more critical — the markdown view also renders fenced code blocks, which scroll horizontally when they exceed the container's content width; at 760px the code area accommodates only ~75 mono chars before scroll, which is short for typical proposal/spec code samples.

At 880px:
- prose at 16px renders ~100 chars per line — past the textbook 66–75 ideal but within practical readability at 4K @ 100% physical sizes, where the eye scans wider measures without losing tracking compared to a 2× Retina display at the same logical px count;
- code blocks at 15px mono accommodate ~97 chars before horizontal scroll, comfortably fitting 80-char-limit code and most 100-col code.

Alternatives considered (deliberated with the user during apply):
- *Body 17, width 820* — first attempt. Width was good, but body at 17px felt head-heavy relative to 15px code blocks. Reverted lg back down 17 → 16 to keep prose and code visually paired.
- *Body 17, width 960+* — preserves the bigger body and gives code even more headroom, but prose at ~107 chars/line drifts past comfortable returning-eye tracking.
- *Body 16, width 820* — comfortable prose but code area shrinks back toward the original problem.
- *Decouple: prose at one width, `<pre>` "breaks out" to a wider container* — cleanest semantic separation but adds layout complexity for marginal gain. Reserved as a future refinement if 880 still feels like a tight code container.

The spec's previous "recommended 720–800px" range is replaced with an explicit 880px anchor plus the prose/code character-per-line targets, so future font-size changes can re-derive the right max-width rather than re-debating a literal pixel value.

### No toggle, no settings persistence

Decided in the exploration phase before this design. A toggle is the safe answer but every visual decision afterwards would have to be tested at two densities, and we have no evidence yet that a single default can't work. If a user reports the new scale is too loose on a 27" Retina, we'll add the toggle then.

## Risks / Trade-offs

- **~30% fewer rows visible in the sidebar at a given height.** A typical sidebar showing 13 rows at 22px each will show ~9–10 rows at 30px each. → Mitigation: the deepest tree per workspace (workspace → change → 4 artifacts) is typically 6–10 rows, so one full workspace still fits in view at common sidebar widths. Accepted.
- **Mono at 12px next to UI at 15px may feel chunky** (mono characters are wider per em than Inter, so the visual weight shifts). → Mitigation: if the chip/count/branch row meta starts to dominate the visual hierarchy in practice, drop `--text-xs` to 11px in a follow-up. Don't pre-emptively split the scale — the simpler uniform bump is preferred until the chunkier mono is actually a problem.
- **Heading h3 = body size** (17 = 17). This relationship already existed before the change (15 = 15); h3 relies on weight 600 alone to register. Not regressed, but worth noting that a future redesign of the markdown view could revisit the heading scale.
- **No rollback complexity** — the change is purely CSS token values + three padding tweaks. Reverting is a single revert commit. No data migrations, no IPC surface change, no persisted state.

## Migration Plan

1. Update `src/App.css` token values and the three padding selectors in a single commit.
2. Update `openspec/specs/visual-identity/spec.md` in the same commit (or via the spec delta in this change directory, then sync at archive time).
3. Run `bun tauri dev` and visually confirm on the developer's 4K @ 100% workstation: sidebar reads comfortably, settings + detail pane track, markdown reader is unchanged in shape.
4. No phased rollout; ship in the next build.

## Open Questions

- None blocking. The "mono at 12px feels chunky" risk is the only candidate for a follow-up adjustment, and we deliberately defer it until we can see the rendered result.
