# Tasks

## 1. Math pipeline

- [ ] 1.1 Pass `{ singleDollarTextMath: false }` to `remarkMath` in `src/components/MarkdownView.tsx` and update the `remarkPromoteStandaloneDisplayMath` doc comment to note its single-dollar branch is now unreachable-but-harmless (`spec-browser`: *Mathematical Notation Rendering*)
- [ ] 1.2 Add `.markdown-view .katex { font-size: 1.05em }` to the math section of `src/App.css`, replacing the vendor 1.21em scale (`spec-browser`: *Mathematical Notation Rendering*)
- [ ] 1.3 Rework `.markdown-view .katex-display` in `src/App.css`: `overflow-x: auto; overflow-y: hidden`, block padding absorbing KaTeX's strut overhang, and an authored `margin: 0.8em 0` overriding katex.min.css (`spec-browser`: *Wide Block Containment*; `visual-identity`: *Markdown Block Rhythm*)
- [ ] 1.4 Migrate this repo's own single-dollar math to `$$…$$`: grep `openspec/**/*.md` for `$…$` inline math and convert each hit in the main specs (verified: `chatgpt-quota`, `document-watch`, `spec-browser`, `touch-input`) and any active change artifacts; archived changes under `openspec/changes/archive/**` are deliberately left unmigrated (historical records — they degrade to literal source text, per design)

## 2. Syntax palette

- [ ] 2.1 Scheme-scope the hljs palette block in `src/App.css`: keep the current literals as dark values inside the `prefers-color-scheme: dark` pattern, add light-well literals for string/number/keyword/type, and verify every token class ≥ 4.5:1 against its scheme's `--surface` code well — including re-checking `#e07a5f` and `var(--text-faint)` comments on the dark well (`visual-identity`: *Syntax Highlight Palette*)

## 3. Tables

- [ ] 3.1 Add a `table` component override in `src/components/MarkdownView.tsx` wrapping tables in `<div className="table-scroll">`, keeping the memo boundary free of new inline props at both call sites (`spec-browser`: *Wide Block Containment*)
- [ ] 3.2 Style `.table-scroll` in `src/App.css` (`overflow-x: auto`, object-tier margin moved off `table`), switch `th` fill to `--surface-3`, and drop table cell text to `--text-md` (`visual-identity`: *Markdown Table Presentation*)

## 4. Mermaid legibility floor

- [ ] 4.1 In `src/components/MermaidBlock.tsx`, after a successful render read the SVG `viewBox` width and set `min-width` to natural × ⅔ so labels never fall below 10px, leaving mermaid's stamped `max-width` intact (`spec-browser`: *Mermaid Diagram Rendering*)
- [ ] 4.2 Add a `ResizeObserver` on the figure frame toggling `figure-frame--reduced` when rendered width < natural width — wired for BOTH figure paths, `src/components/MermaidBlock.tsx` and `src/components/SvgBlock.tsx` (a shared hook or frame component) — and hold `.figure-maximize` at `opacity: 1` under that class in `src/App.css` (`spec-browser`: *Maximized Figure View*)

## 5. Rhythm, measure, footnotes

- [ ] 5.1 Add `.markdown-view pre { margin: 0.8em 0 }` so fenced code joins the object tier instead of riding the UA default (`visual-identity`: *Markdown Block Rhythm*)
- [ ] 5.2 Apply the prose measure in `src/App.css`: `max-width: 74ch` on `.markdown-view p`, `ul`, `ol`, `blockquote`; headings and object blocks keep the 880px column (`visual-identity`: *Markdown Body Adopts the Type System*)
- [ ] 5.3 Style `section[data-footnotes]` in `src/App.css` — top rule, clear top spacing, `--text-sm` at `--text-muted` — and add a real `.sr-only` utility rule so the GFM "Footnotes" heading is hidden deliberately (`visual-identity`: *Markdown Block Rhythm*)
- [ ] 5.4 Add `.markdown-view img { max-width: 100% }` in `src/App.css` so document images can never widen the column — today no `img` rule exists at all (`spec-browser`: *Wide Block Containment*; groundwork for the deferred image-resolution change)

## 6. Verification

- [ ] 6.1 `bun run build` (type-check + fresh `dist/`) and `cargo test` stay green (frontend-only change — the mutation gate short-circuits by design; coverage here is the browser-loop walk below)
- [ ] 6.2 Browser-loop smoke against a torture artifact (isolated `specforge-serve` + scratch workspace, per the audit's method): walk every new/changed spec scenario — prose dollar amounts render literally; `$$…$$` inline sits at prose size; display math shows no vertical scrollbar and no clipped limits (`scrollHeight === clientHeight`); the wide table scrolls in its block while the pane does not; the 2579px flowchart holds ≥10px labels and scrolls; its maximize control is visible at rest; footnotes sit below a rule
- [ ] 6.3 Repeat the checks that are scheme-sensitive in both light and dark (hljs token contrast ≥ 4.5:1 measured against each code well; math/table/figure treatments hold in both), and run a `bun tauri dev` smoke to confirm the native shell renders identically
