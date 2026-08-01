# SVG Fence Rendering

## Why

Artifact authors (frequently agents) can only express diagrams through `mermaid` fences today; a raw `<svg>` block is silently dropped by the renderer and an `svg` fence shows up as highlighted source rather than a picture. SVG fences would let a design doc embed any diagram mermaid cannot express — precise layouts, custom geometry, annotated mockups — while staying fully self-contained (no side files, no relative paths that break across archive moves), degrading on GitHub to a readable code block.

## What Changes

- Fenced code blocks with the `svg` info string render as images in the rich frontend's markdown view, following the same interception pattern the `mermaid` fence already uses.
- Rendering is structurally inert: the SVG source becomes a data-URI `<img>`, so scripts, event handlers, and external fetches inside fence content can never execute — the same security posture the mermaid requirement demands, achieved without a sanitizer dependency.
- A rewrite pass makes naïve fences Just Work: missing `xmlns` is injected (required for a standalone image document, routinely omitted by authors), missing `width`/`height` are derived from the `viewBox` so WKWebView doesn't fall back to arbitrary replaced-element defaults, and the live `--text` design token is injected as the root `color` (when the author didn't declare one) so `currentColor` diagrams follow the light/dark scheme.
- Invalid SVG degrades gracefully to the fence's raw source with a quiet note, mirroring the mermaid fallback; the rest of the artifact renders normally.
- Every other fenced code block, including `xml`, is unaffected; the raw artifact markdown returned by the backend is unchanged; the `terminal-ui` frontend continues to present `svg` fences as code text.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `spec-browser`: adds an "SVG Fence Rendering" requirement — `svg` fences render as inert images with graceful degradation, theme-aware `currentColor` resolution, and no active content — and modifies the existing "Mermaid Diagram Rendering" requirement, scoping its blanket "every other fenced code block" clause to info strings not special-cased by the capability so the two requirements don't contradict once merged.

## Impact

- **Frontend only.** `src/components/MarkdownView.tsx` (the `<pre>` override and the rehype-highlight `plainText` exemption list) and a new `SvgBlock` component beside `MermaidBlock.tsx`; shared layout/fallback CSS in `src/App.css`.
- **No Rust, IPC, or dependency changes.** No new npm packages — validation and rewriting use the WebView's built-in `DOMParser`/`XMLSerializer`.
- **Specs.** `openspec/specs/spec-browser/spec.md` gains one requirement, and the existing Mermaid requirement's "every other fenced code block" clause is narrowed to exclude special-cased info strings; no other capability is touched.
