# Mathematical Notation Rendering — Tasks

## 1. Dependencies

- [ ] 1.1 Add `katex`, `remark-math@^6`, and `rehype-katex@^7` to `package.json` dependencies via `bun add` (unified-11-compatible majors, matching `react-markdown@9`)

## 2. Frontend Implementation

- [ ] 2.1 Wire dollar-math into the pipeline in `src/components/MarkdownView.tsx`: add `remarkMath` to `remarkPlugins` and `rehypeKatex` (with `throwOnError: false`, `trust: false`, `strict: "ignore"`) to `rehypePlugins` after `rehype-highlight`; import `katex/dist/katex.min.css` once (`spec-browser`: *Mathematical Notation Rendering* — inline and display dollar syntax, code spans exempt by the parser)
- [ ] 2.2 Create `src/components/MathBlock.tsx`: renders a fence body via `katex.renderToString(source, { displayMode: true, throwOnError: false, trust: false, strict: "ignore" })`, catching render failure to the `fence-block--error` raw-source + quiet-note fallback used by `MermaidBlock` (`spec-browser`: *Mathematical Notation Rendering* — math fence and its degradation)
- [ ] 2.3 Intercept the ```math fence in `MarkdownView.tsx`'s `pre` override via `fenceSource(node, "math")` → `MathBlock`, third alongside `mermaid`/`svg`; extend the `HIGHLIGHT_OPTIONS` comment to note why `math` needs no `plainText` exemption (`spec-browser`: *Mermaid Diagram Rendering* and *SVG Fence Rendering* — updated special-case enumerations)
- [ ] 2.4 Style in `src/App.css`, scoped under `.markdown-view`: `.katex-display { overflow-x: auto }` so wide formulas scroll inside their block, and `.katex-error { color: var(--warn) !important }` overriding KaTeX's baked inline `errorColor` with the token (`spec-browser`: *Mathematical Notation Rendering* — display overflow and quiet error indication)

## 3. Verification

- [ ] 3.1 Run `cargo test` (workspace stays green — no Rust changes expected, confirming the frontend-only boundary)
- [ ] 3.2 Run `bun run build` (strict tsc + bundle; confirms plugin/type compatibility and that KaTeX assets bundle)
- [ ] 3.3 Manual smoke in `bun run wt:dev` with a scratch artifact walking the spec scenarios: inline `$O(n \log n)$` in prose, display `$$…$$`, a ```math fence, backticked `` `\\wsl$\Ubuntu` `` and `` `${tag}` `` staying literal, an unpaired `$`, invalid LaTeX between dollars (in-place source with `--warn` indication), an invalid ```math fence (source + quiet note), `\href{https://example.com}{x}` producing no live link, a wide formula scrolling inside its block, and light/dark both rendering math in the surrounding text colour (`spec-browser`: *Mathematical Notation Rendering* — all scenarios)
- [ ] 3.4 Confirm rendered math carries MathML output alongside the visual spans (inspect the DOM for `<math>` with the TeX annotation) (`spec-browser`: *Mathematical Notation Rendering* — machine-readable representation)
