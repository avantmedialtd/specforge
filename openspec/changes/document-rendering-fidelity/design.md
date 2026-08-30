# Document Rendering Fidelity — Design

## Context

A rendered-output audit (torture-test artifact through `specforge-serve`, both schemes, computed-style cross-checks) established the defect list this change addresses. The rendering pipeline is `react-markdown` + `remark-gfm`/`remark-math` + `rehype-highlight`/`rehype-katex` in `MarkdownView.tsx`, styled by the markdown-renderer section of `App.css`, with `MermaidBlock`/`SvgBlock` intercepting fences at the `<pre>` component override. Three surfaces share the component (detail pane, reader windows, file-browser previews), so every fix lands once.

Measured baseline, for reference during implementation: inline KaTeX computes 19.36px inside 16px prose; all four torture display formulas clip vertically (`scrollHeight` 3–6px over `clientHeight`); a 2579px-natural flowchart renders at scale 0.34 → 5.1px labels; the 10-column table renders 982px wide and overhangs the 880px column; `pre` margin is the UA's 15px and `.katex-display`'s is vendor katex.css's 16px against authored tiers of 9.6/12.8px; hljs strings `#6cc77a` measure ~1.9:1 on the light code well.

## Goals / Non-Goals

**Goals:**

- Every audit defect in scope closed at the layer that owns it: parser options (`MarkdownView`), CSS (`App.css`), or the diagram block (`MermaidBlock`).
- No behavioral change to the security posture: KaTeX stays untrusting, mermaid stays strict, SVG stays image-context.
- All three markdown surfaces improve identically; `specforge-tui` untouched.

**Non-Goals:**

- Workspace-relative image resolution (needs a served-file endpoint + asset-protocol work across crates — its own change).
- Raw-HTML rendering (`<details>` etc. — a security-posture decision interlocking with the link/KaTeX trust stance — its own change).
- Any change to `openspec-core` / `openspec-app`.

## Decisions

### Decision 1: Drop single-dollar math (`singleDollarTextMath: false`)

`remarkMath` gains `{ singleDollarTextMath: false }`. Inline math remains fully available — `$$…$$` embedded in prose already renders inline (spec'd and shipped), and the `remarkPromoteStandaloneDisplayMath` plugin's source-offset check keeps working unchanged (its single-dollar branch simply becomes unreachable, and stays harmless).

- **Rejected: keep GitHub-compatible `$…$`.** The false positive silently corrupts prose ("costs $50 per seat and $60" → typeset garbage), and a spec-reading tool's first duty is to never mangle the document. GitHub compatibility is worth less than prose safety here, and the migration story (`$…$` → `$$…$$`) is mechanical.
- **Rejected: heuristic demotion** (a remark post-pass un-mathing "price-like" inlineMath nodes). Unbounded edge cases, and a parser that sometimes treats `$…$` as math is worse to reason about than one that never does.

**BREAKING migration**: grep all registered-workspace-style docs in this repo (`openspec/**/*.md`) for single-dollar math during implementation and convert to `$$…$$`. Verified hits in the main specs: `chatgpt-quota`, `document-watch`, `spec-browser`, and `touch-input` (this change's own artifacts are already authored in post-migration form). Archived changes under `openspec/changes/archive/**` also contain single-dollar math and are deliberately NOT migrated: they are historical records, and the post-change degradation — rendering as literal source text — is exactly the safe behavior the new contract promises.

### Decision 2: KaTeX size and clipping are fixed in CSS, not options

`.markdown-view .katex { font-size: 1.05em }` replaces the vendor 1.21em (a hair above 1.0 reads best against Inter's x-height); `.markdown-view .katex-display` gets `overflow-x: auto; overflow-y: hidden` plus enough block padding to absorb KaTeX's strut overhang, verified empirically in the browser loop (`scrollHeight === clientHeight` on the torture formulas — tall limits, deep subscripts). Margins move to the object tier per Decision 5.

- **Rejected: KaTeX `minRuleThickness`/option-level sizing** — KaTeX has no supported option for the base scale; the documented lever is exactly this CSS override.
- **Rejected: leaving `overflow-y` auto with a taller line-height** — line-height doesn't absorb strut overhang reliably across formulas; measured deltas ranged 3–6px.

### Decision 3: Mermaid legibility floor via measured `min-width`

$$s_{\min} = \frac{f_{\min}}{f_{\text{label}}} = \frac{10}{15} = \tfrac{2}{3}$$

After a successful render, `MermaidBlock` reads the SVG's `viewBox` width $$W_n$$ and its *measured* label size, then publishes `⌈W_n · s_min⌉px` as a `--figure-floor` custom property on `.mermaid-block`; `.mermaid-block > svg { min-width: var(--figure-floor, 0) }` applies it, alongside mermaid's own stamped `max-width: W_n px`.

**The property, not `svg.style.minWidth` — learned in implementation.** Setting the inline style imperatively on the injected SVG works exactly once: the first `reduced` flip re-renders this component, React re-establishes the `dangerouslySetInnerHTML` subtree, and the imperative style is gone — leaving the *wide* diagram (the only one that flips) as the single case where the floor silently failed. A value React owns survives its own re-renders. The label size still has to be measured in an effect, because it is only knowable once the diagram is in the document; only its *application* moves into CSS.

$$f_{\text{label}}$$ is read from the rendered diagram rather than assumed: flowcharts label at the theme's `--text-md` (15px) while sequence diagrams use 16px, so a hardcoded ⅔ would floor the two diagram types at different label sizes. The existing `.mermaid-block { overflow-x: auto }` — today a documented safety net — becomes the load-bearing scroll surface exactly and only when the floor engages. A `ResizeObserver` on the figure frame compares rendered to natural width and toggles `figure-frame--reduced`, which `App.css` uses to hold the maximize affordance at `opacity: 1` (the touch-input at-rest rules already establish that pattern). The observer lives with the figure frame, which both `MermaidBlock` and `SvgBlock` render — so a reduced SVG image earns the at-rest affordance the same way a reduced diagram does, matching the *Maximized Figure View* delta's "any figure rendered below its natural size". The legibility floor's `min-width` itself stays mermaid-only: an authored SVG's text was sized by its author against its own viewBox, so the engine-label heuristic does not transfer.

- **Rejected: `useMaxWidth: false` engine-wide** — loses fit-to-pane for the common moderate diagram, which the spec deliberately keeps.
- **Rejected: pure-CSS floor** — CSS cannot read the SVG's natural width; the block already owns a post-render measurement point, so the JS is one line where the data is.

### Decision 4: Tables contained by a component-level wrapper

`MarkdownView` adds a `table` component override rendering `<div className="table-scroll"><table …/></div>`, with `.table-scroll { overflow-x: auto }` carrying the object-tier margin (the table's own margin moves to the wrapper so the scrollbar hugs the table). Header tier: `th { background: var(--surface-3) }` separates from the `--surface-2` zebra; cells drop to `var(--text-md)`.

- **Rejected: `table { display: block; overflow-x: auto }`** — destroys table display semantics (accessibility tree, caption behavior, centering) for a purely visual gain.
- **Rejected: status quo (pane pans)** — the audit's measured 982px table overhangs the column today and pans the whole document on narrow panes; that violates the new *Wide Block Containment* requirement.

### Decision 5: Rhythm ownership — two tiers, no vendor margins, footnotes finished

Every object block — `pre`, `.katex-display`, `.table-scroll`, `.mermaid-block`, `.svg-block`, `blockquote` — carries `margin: var(--space-3) 0`; paragraphs/lists stay on the em-based prose tier. `section[data-footnotes]` gets a top rule, `2em` top spacing, and `--text-sm` at `--text-muted`. Prose measure per the spec: `p, ul, ol, blockquote { max-width: 74ch }` inside the 880px column; headings and object blocks span the column.

**The token, not `0.8em` — learned in implementation.** The plan was to give `pre` and `.katex-display` the same `0.8em` the other object blocks already had. But `em` resolves against the element's *own* font size, and `pre` sets its own (`--text-md`): the identical declaration computed 12.8px on a table and 12px on a code fence. A tier that isn't one number isn't a tier, so the whole object tier moved to `var(--space-3)` (12px), which computes the same wherever it lands. The 0.8px shift on the blocks that already had `0.8em` is imperceptible and buys an actually-uniform rhythm.

**`.sr-only` was already there.** The audit read the GFM footnotes heading as "accidentally hidden by third-party CSS" and this design planned to add the utility. It is in fact declared deliberately in `App.css` (written for the tree's screen-reader-only favorite state) and already covers the heading — so the fix is one rule smaller than planned, and adding it would have been a duplicate.

- **Rejected: GitHub's uniform 16px gap for everything** — flattens the existing deliberate prose/object distinction that the rest of the CSS already implements; finishing the two-tier system is the smaller, more coherent change.
- **Rejected: narrowing the whole column to the measure** — starves tables, code, and diagrams of width they measurably need (the audit's table wants 982px); the two-tier column keeps both audiences whole.

### Decision 6: Syntax palette goes scheme-scoped, stays literal

The four literal token colours stay in the sanctioned palette block but split per scheme: the existing hexes remain the dark values (they were tuned on the dark well and all clear 4.5:1 there — verify types `#e07a5f` and retune if it misses); a new set of darker literals is chosen for the light well (same hue identities: green string, amber number, purple keyword, red-orange type), each verified ≥ 4.5:1 against light `--surface` (#fff). Scoped with the same `prefers-color-scheme` pattern every other scheme-varying rule uses.

- **Rejected: promoting to `--hljs-*` tokens** — equally correct, but spreads six single-consumer names into the global token namespace the *Design Token Layer* requirement enumerates; the palette carve-out exists precisely so this block can hold its literals.

## Risks / Trade-offs

- **[BREAKING delimiter change strands existing `$…$` docs]** → Migration grep in this repo is part of the tasks; for external workspaces the failure mode is benign — the source renders as literal text, exactly what the prose-safety contract promises, and `$$…$$` is a one-character-per-side fix.
- **[1.05em inline math may look small for dense scripts (nested fractions)]** → The value is a starting point verified against the torture doc in the browser loop; the spec pins "harmonized with prose", not a number, so tuning stays in-contract.
- **[The prose measure changes every document's look at once]** → It's one `max-width` declaration — trivially tuned or reverted; the spec's 70–80ch range gives latitude without a spec change.
- **[`min-width` on the mermaid SVG can fight `useMaxWidth`'s stamped inline style]** → Verified in the browser loop with the audit's 2579px flowchart: floor engages, block scrolls, pane does not; the lightbox path is unaffected because it re-renders at its own scale.
- **[MarkdownView memo contract]** — the new `table` override and any floor-related state MUST NOT introduce inline object/callback props at the `MarkdownView` call sites; the memo boundary is a documented correctness prerequisite for the detail pane's equality guard. All new state stays inside `MermaidBlock`/`MarkdownView`'s module scope, matching how `maximized` state is handled today.
