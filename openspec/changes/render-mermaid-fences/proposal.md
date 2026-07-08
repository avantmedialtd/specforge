# Render Mermaid Diagrams from `mermaid` Code Fences

## Why

SpecForge renders every OpenSpec artifact — proposal, design, tasks, and capability specs — as markdown in the detail pane, and design docs lean on diagrams to explain architecture, state machines, and data flow. Today those diagrams must be hand-drawn in ASCII inside a fenced block, or authored elsewhere and pasted as an image the app cannot show. A ` ```mermaid ` fence — the de-facto standard that GitHub, GitLab, and Obsidian all render — currently falls through to `rehype-highlight` and shows up as unhighlighted source text, not a picture.

The gap is small but real, and the ground is unusually favourable: the app already parses and renders the exact markdown these diagrams live in, the rendering pipeline already overrides element renderers, and — crucially — the WebView runs with **no Content-Security-Policy**, so Mermaid's inline `<style>` injection and dynamic rendering have nothing to fight. The one thing standing between a design doc's diagram source and a rendered diagram is a `code`-fence interceptor.

## What Changes

- The detail pane's markdown renderer (`MarkdownView`) gains a `code` component override that detects the `mermaid` info string and renders the fence as a **graphical diagram** instead of highlighted source. Every other fenced language renders exactly as before.
- Diagrams are **themed from the app's design tokens** — Mermaid runs on its `base` theme with `themeVariables` populated at runtime from the CSS custom properties (`--surface`, `--accent`, `--border-strong`, `--text`, `--font-mono`, …), so a diagram reads as part of the same surface as the surrounding prose in both light and dark. A `prefers-color-scheme` change re-renders visible diagrams.
- Rendering is **resilient**: invalid diagram source degrades to the raw fence plus a quiet "couldn't render" note; a Mermaid parse failure never blanks the pane or leaks Mermaid's default error graphic into the document.
- Mermaid loads **lazily** (`import('mermaid')` on the first diagram encountered) so its ~2.8 MB pulls into a separate chunk and never weighs down the initial bundle.
- Diagrams render under Mermaid's **strict security level** (DOMPurify on, click/script handlers off) — the right default given the app carries no CSP backstop.

## Capabilities

### Modified Capabilities

- `spec-browser`: the detail pane gains a new *Mermaid Diagram Rendering* behaviour — `mermaid` fences render as design-token-themed diagrams, other fences render unchanged, invalid diagrams degrade to source, and rendering is strict-security and client-side only.

## Impact

- `package.json` — add `mermaid` as a dependency (lazy-imported, code-split by Vite).
- `src/components/MarkdownView.tsx` — add a `code` renderer override that routes `language-mermaid` to a new `MermaidBlock`; ensure the raw fence source reaches it intact (keep `mermaid` out of what `rehype-highlight` tokenises).
- New `src/components/MermaidBlock.tsx` — a client component that lazy-loads Mermaid, initialises it once with token-mapped `themeVariables` + `securityLevel: 'strict'` + `suppressErrorRendering`, renders the SVG asynchronously with a unique id, guards against unmount/stale-content races, re-renders on `prefers-color-scheme` change, and falls back to source on error.
- `src/App.css` — minimal container styling for the diagram block (centring, overflow handling for wide diagrams) consistent with the existing `.markdown-view pre` treatment.
- **No Rust changes.** `read_artifact` still returns raw markdown verbatim; this is a pure frontend concern.
- **Deliberate scope boundaries** (so nobody "fixes" these later as oversights):
  - **Rich frontend only.** The desktop WebView and the shared `web-ui` browser bundle render diagrams; the `terminal-ui` frontend cannot paint SVG and continues to show `mermaid` fences as code text. This is a non-goal, not an oversight.
  - **No diagram authoring aids.** No live preview, no editing, no diagram picker — the app is a read-only viewer of markdown that happens to contain diagrams.
  - **No pan/zoom in v1.** A wide diagram scales down to fit the pane width rather than scrolling; interactive pan/zoom and click-to-expand are a possible fast-follow, not a gate.
  - **Not a new syntax-highlight theme.** The hand-written `.hljs-*` palette is untouched; Mermaid theming is a separate runtime concern that reads the same tokens.
