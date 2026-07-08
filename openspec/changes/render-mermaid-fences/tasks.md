## 1. Dependency + pipeline seam

- [ ] 1.1 Add `mermaid` to `package.json` dependencies (do not eagerly import it anywhere)
- [ ] 1.2 In `MarkdownView.tsx`, configure `rehype-highlight` so the `mermaid` language is left untouched (raw source preserved for the `code` override)
- [ ] 1.3 Add a `code` renderer override that detects the `mermaid` info string / `language-mermaid` class and delegates to `<MermaidBlock source={…} />`, leaving all other fences on the existing highlighted path

## 2. `MermaidBlock` component

- [ ] 2.1 Create `src/components/MermaidBlock.tsx` that lazy-loads Mermaid via `await import('mermaid')` on first render and initialises it once (module-level guard) with `theme: 'base'`, `securityLevel: 'strict'`, and `suppressErrorRendering: true`
- [ ] 2.2 Build the token-mapped `themeVariables` by reading the app's CSS custom properties via `getComputedStyle(document.documentElement)` (background/surface, accent, border-strong, text, mono font)
- [ ] 2.3 Render asynchronously with a unique id (`useId`), injecting the returned SVG; guard the effect against unmount / stale-source with an `ignore` flag
- [ ] 2.4 Subscribe to `matchMedia('(prefers-color-scheme: dark)')` and re-render the diagram on scheme change so baked-in colours follow the theme
- [ ] 2.5 On parse/render failure, fall back to the raw source in a `<pre>` with a quiet "couldn't render diagram" note; ensure Mermaid's default error graphic never reaches the DOM

## 3. Styling

- [ ] 3.1 Add minimal `.markdown-view` diagram-container styling in `App.css` (block layout, centring, horizontal overflow scroll for wide diagrams) consistent with the existing `pre` treatment; no literal colours (diagram colours come from tokens at runtime)

## 4. Verification

- [ ] 4.1 Author a scratch artifact with a valid flowchart, a valid sequence diagram, a non-mermaid code fence, and a deliberately broken `mermaid` fence; confirm in `bun tauri dev` that diagrams render, other fences stay highlighted, and the broken one degrades to source
- [ ] 4.2 Toggle the OS between light and dark with a diagram visible and confirm it re-renders in the active scheme's tokens
- [ ] 4.3 Confirm the initial bundle does not include Mermaid (it lands in a separate chunk) via the Vite build output
- [ ] 4.4 `bun run build` passes (tsc strict + bundle)
