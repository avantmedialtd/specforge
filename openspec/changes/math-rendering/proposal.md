# Mathematical Notation Rendering

## Why

OpenSpec artifacts in technical workspaces routinely contain mathematical notation — complexity bounds, formulas, invariants — and today SpecForge shows the LaTeX source as plain prose. GitHub renders math in GFM (`$…$`, `$$…$$`, and ```` ```math ```` fences), and the spec-browser capability already promises GFM rendering, so authors writing for GitHub reasonably expect the same notation to render here.

## What Changes

- The detail pane renders GitHub-flavored math via KaTeX: inline `$…$`, display `$$…$$` (through `remark-math` + `rehype-katex` in the existing `react-markdown` plugin chain), and ```` ```math ```` fences as display math (through the existing `<pre>` fence-interception pattern, sibling to `mermaid` and `svg`).
- Invalid LaTeX degrades gracefully: an invalid dollar-delimited expression renders its source in place with a quiet error indication while the rest of the artifact renders normally; an invalid ```` ```math ```` fence falls back to its raw source, matching the invalid-mermaid treatment.
- Math rendering runs with KaTeX's non-trusting posture: LaTeX commands that would emit active content (e.g. `\href`) do not produce live links or scripts.
- KaTeX and its fonts/CSS are bundled locally and loaded eagerly (no CDN, works offline); fonts fetch only when math actually renders.
- Rendered math follows the active color scheme with no extra theming plumbing (KaTeX renders in `currentColor`).
- The `terminal-ui` frontend is deliberately unaffected: it continues to present math source as text, mirroring the existing mermaid/svg carve-outs. The web UI inherits the behavior automatically by serving the same frontend bundle.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `spec-browser`: adds a *Mathematical Notation Rendering* requirement — sibling to the *Mermaid Diagram Rendering* and *SVG Fence Rendering* requirements — covering the three GitHub math syntaxes, graceful degradation of invalid input, the no-active-content posture, and the client-side / raw-markdown-unchanged / terminal-ui carve-outs.

## Impact

- **Frontend only** — `src/components/MarkdownView.tsx` (plugin chain + fence interception), a new `src/components/MathBlock.tsx`, `src/App.css` (display-math overflow scrolling, error-color token override), and `package.json` (adds `katex`, `remark-math`, `rehype-katex`).
- **Deliberately does NOT change**: no Rust crates, no IPC surface, no backend behavior — the raw artifact markdown returned by `read_artifact` is unchanged. `openspec-core`'s parser is untouched, so tree-pane task labels continue to show raw markdown source (including any LaTeX), exactly as they do for bold or backticked text today. `crates/specforge-tui` is untouched. No lazy-loading machinery is added for KaTeX (the mermaid lazy-load remains the exception, justified by its 2.8 MB chunk).
