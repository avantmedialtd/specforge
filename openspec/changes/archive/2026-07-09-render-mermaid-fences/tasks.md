## 1. Dependency + pipeline seam

- [x] 1.1 Add `mermaid` to `package.json` dependencies (do not eagerly import it anywhere)
- [x] 1.2 In `MarkdownView.tsx`, pass `plainText: ['mermaid']` to `rehype-highlight` so the fence body is left as a raw text node (source preserved for the override)
- [x] 1.3 Add a `pre` renderer override that detects a child `code` node carrying the `language-mermaid` class and delegates to `<MermaidBlock source={…} />`, leaving all other fences on the existing highlighted path. (Overriding `pre` rather than `code` avoids nesting an `<svg>` inside a `<pre>` and inheriting the code-well styling — see design.md.)

## 2. `MermaidBlock` component

- [x] 2.1 Create `src/components/MermaidBlock.tsx` that lazy-loads Mermaid via `await import('mermaid')` on first render (module-level guard on the import promise) and calls `initialize` with `theme: 'base'`, `securityLevel: 'strict'`, and `suppressErrorRendering: true`. `initialize` runs per render, not once, because it carries the scheme-dependent `themeVariables`.
- [x] 2.2 Build the token-mapped `themeVariables` by reading the app's CSS custom properties via `getComputedStyle(document.documentElement)` (background/surface, accent, border-strong, text, mono font)
- [x] 2.3 Render asynchronously with a unique id derived from `useId()` (stripped to alphanumerics — `useId` emits colons, which are invalid in Mermaid's internal `#id` selectors — plus a per-attempt counter for StrictMode's double-invoked effects); inject the returned SVG and guard the effect against unmount / stale-source with an `ignore` flag
- [x] 2.4 Subscribe to `matchMedia('(prefers-color-scheme: dark)')` and re-render the diagram on scheme change so baked-in colours follow the theme
- [x] 2.5 On parse/render failure, fall back to the raw source in a `<pre>` with a quiet "couldn't render diagram" note; ensure Mermaid's default error graphic never reaches the DOM

## 3. Styling

- [x] 3.1 Add minimal `.markdown-view` diagram-container styling in `App.css` (block layout, centring, horizontal overflow scroll for wide diagrams) consistent with the existing `pre` treatment; no literal colours (diagram colours come from tokens at runtime)

## 4. Verification

- [x] 4.1 Author a scratch artifact with a valid flowchart, a valid sequence diagram, a non-mermaid code fence, and a deliberately broken `mermaid` fence; confirm in `bun tauri dev` that diagrams render, other fences stay highlighted, and the broken one degrades to source
- [x] 4.2 Toggle the OS between light and dark with a diagram visible and confirm it re-renders in the active scheme's tokens
- [x] 4.3 Confirm the initial bundle does not include Mermaid (it lands in a separate chunk) via the Vite build output
- [x] 4.4 `bun run build` passes (tsc strict + bundle)
