# Mathematical Notation Rendering — Design

## Context

`MarkdownView.tsx` is the single markdown renderer for every rich surface — active artifacts (`DetailPane`), archived artifacts (`ArchiveView` embeds `DetailPane`), and workspace file previews (`FileBrowserView`) — and the web UI serves the same bundle, so one integration point covers all of them. The pipeline is `react-markdown@9` (unified 11) with `remark-gfm` + `rehype-highlight`, plus `<pre>`-level interception for `mermaid` and `svg` fences. Math is the first inline-capable construct added to this pipeline: `$O(n \log n)$` mid-sentence cannot be handled by fence interception, which forces the plugin route for dollar syntax. Tauri's CSP is `null` and all assets bundle locally, so fonts and CSS ship with the app and work offline.

## Goals / Non-Goals

**Goals:**

- Render the three GitHub math syntaxes — inline `$…$`, display `$$…$$`, ```` ```math ```` fences — in every `MarkdownView` surface, both transports.
- Degrade invalid input in place, quietly, without disturbing the rest of the artifact.
- No active content from math source; no network access at render time.
- Zero theming plumbing: math follows the active scheme through CSS inheritance.

**Non-Goals:**

- No Rust/IPC/backend changes; raw artifact markdown is untouched.
- No math in the tree pane's task labels (they remain raw parser text, as for bold/backticks today).
- No `terminal-ui` rendering (carve-out mirrors mermaid/svg).
- No lazy-loading machinery for the math engine.
- No custom LaTeX macro packages or user-configurable math settings in v1.

## Decisions

### Engine: KaTeX

`katex` rendered through the unified plugin ecosystem. Fast, synchronous, battle-tested, output is inert HTML spans + MathML, themes via `currentColor`, and fonts bundle locally.

- **Rejected — MathJax 4 (SVG output):** fullest LaTeX coverage and no font shipping, but several times heavier and slower; spec documents don't need the coverage delta.
- **Rejected — Temml → native MathML:** lightest option, but rendering fidelity diverges between WebKit (desktop WKWebView) and Chromium (web UI serves the same bundle), and its unified integrations are less maintained.
- **Rejected — Typst:** nicer syntax but not LaTeX — no GitHub-authored document uses it, and typst.ts means a heavy WASM bundle.

### Dollar syntax via `remark-math` + `rehype-katex` plugins

`remark-math@6` parses `$…$` / `$$…$$` into `inlineMath`/`math` nodes at the remark stage; `rehype-katex@7` renders them at the rehype stage (after `rehype-highlight` in the plugin array; the two touch disjoint nodes). Versions match the unified 11 ecosystem `react-markdown@9` uses.

- **Rejected — fence-only support:** cannot render inline math inside prose, which is the primary authoring pattern.
- **Rejected — regex preprocessing of the markdown string:** fragile around code spans/fences; remark-math already skips code correctly at the parser level.

### Single-dollar inline math stays enabled (GitHub parity)

Default `remark-math` behaviour. The delimiter rules are conservative (no whitespace adjacent to the closing `$`), code spans are exempt, and an audit of the active corpus found every existing `$` either inside backticks or unpaired — zero silent reinterpretation on ship day.

- **Rejected — `singleDollarTextMath: false`:** eliminates the residual false-positive risk in arbitrary file previews but diverges from what authors write on GitHub; can be flipped later without affecting `$$` or fences if practice proves otherwise.

### ```math fence via the existing `<pre>` interception

`remark-math` does not handle fenced math, so the `<pre>` component override gains a third case: `fenceSource(node, "math")` → new `MathBlock` component calling `katex.renderToString(source, { displayMode: true, throwOnError: false, trust: false })`. `rehype-highlight` ignores grammars it doesn't register (`detect` is off) and `fenceSource` reconstructs the text either way, so no `plainText` exemption is needed — the `HIGHLIGHT_OPTIONS` comment gets a short addendum instead.

- **Rejected — a remark-stage fence transform:** would duplicate a dispatch mechanism the component layer already owns for `mermaid`/`svg`; consistency wins.

### Eager bundling

`katex` + plugins import statically; Vite chunks them with the main bundle (~80 KB gz JS + 23 KB CSS; fonts are fetched by CSS only when math actually renders). `katex/dist/katex.min.css` is imported once in `MarkdownView.tsx`.

- **Rejected — mermaid-style lazy load:** justified for mermaid's 2.8 MB chunk, not here. A lazy *plugin* also makes the plugin array stateful and flashes raw dollars mid-sentence during the async swap — inline math has no clean "pending box" like a block-level diagram.

### Error handling in two tiers

Dollar math renders with `throwOnError: false`: KaTeX emits the offending source in place inside a `.katex-error` span, and the rest of the document is untouched. A failing `MathBlock` fence falls back to the existing `fence-block--error` treatment (raw source + quiet note), identical to invalid mermaid. `strict: "ignore"` suppresses KaTeX's console warnings for benign non-strict LaTeX.

- **Rejected — letting errors throw:** a single typo would take down the whole artifact render, violating the graceful-degradation requirement.

### Colours through tokens, not options

KaTeX output inherits `currentColor`, so light/dark needs nothing. KaTeX bakes `errorColor` as an inline `style` attribute, so instead of passing a literal colour through options, CSS overrides it: `.markdown-view .katex-error { color: var(--warn) !important; }` — honouring the no-literal-colours-in-components invariant. `.katex-display` gets `overflow-x: auto` so a wide formula scrolls inside its own block instead of widening the pane.

- **Rejected — `errorColor` option:** would hardcode a hex literal in the component and drift from the token when schemes change.

### Accessibility via default output

KaTeX's default `output: "htmlAndMathml"` embeds a MathML rendering (with the original TeX as an annotation) alongside the visual spans; assistive technology consumes the MathML. No configuration needed — the decision is to *not* switch to `output: "html"` to shave bytes.

## Risks / Trade-offs

- **[Single-dollar false positives in arbitrary previews]** `FileBrowserView` renders any workspace markdown, where money-shaped dollars are likelier than in specs. → Conservative delimiter rules + code-span exemption cover most cases; a mangled line renders as odd math but never blanks the pane; `singleDollarTextMath: false` remains a one-line escape hatch.
- **[KaTeX coverage gaps]** Some LaTeX (rare environments, packages) is unsupported. → `throwOnError: false` shows the source in place; the artifact stays readable.
- **[Bundle growth in the initial chunk]** ~100 KB gz added eagerly. → Accepted deliberately; fonts stay lazy (CSS-triggered), and the mermaid precedent remains the exception for genuinely heavy engines.
- **[Ecosystem version drift]** `remark-math`/`rehype-katex` majors track unified majors. → Pin the unified-11-compatible majors (`remark-math@6`, `rehype-katex@7`); `bun run build`'s strict tsc surfaces breakage at upgrade time.
- **[KaTeX CSS collides with app styles]** Global stylesheet import. → KaTeX namespaces everything under `.katex`; overrides are scoped under `.markdown-view`.
