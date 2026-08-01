# Tasks: SVG Fence Rendering

## 1. Sizing spike (verify D4's WKWebView assumption early)

- [x] 1.1 Add a scratch markdown file with three `svg` fences — absolute `width`/`height`, `viewBox`-only, and neither — to a throwaway registered workspace, run the app via `bun run wt:dev`, and record how the WebView sizes a data-URI `<img>` in each case
- [x] 1.2 Confirm the viewBox-derived sizing rule (one user unit per CSS pixel) against the measurements; if reality disagrees with design.md D4, update D4 before building the rewrite pass

## 2. SvgBlock component

- [x] 2.1 Create `src/components/SvgBlock.tsx` with the validity gate per D3: parse the fence body with `DOMParser("image/svg+xml")` and accept only documents with no `parsererror` whose root localName is `svg` in the SVG namespace or no namespace (a missing `xmlns` parses cleanly — it is normalized later, not rejected); route everything else to a fallback that reuses the `mermaid-block--error` presentation (quiet note + raw source)
- [x] 2.2 Implement the rewrite pass on the parsed document: inject `xmlns` when absent; derive root `width`/`height` from `viewBox` when absolute dimensions are missing; when the root declares no `color` (attribute or inline style), set the live `--text` token (read off `:root`) as the root's `color` so `currentColor` resolves to it by inheritance; extract a root-level `<title>` for the img `alt`, else a generic label
- [x] 2.3 Serialize with `XMLSerializer`, URI-encode into a `data:image/svg+xml` src, and render as `<img>`; wire `onError` to the same source fallback as the parse gate
- [x] 2.4 Re-key the (fully synchronous) memo on `prefers-color-scheme` changes using the same `DARK_SCHEME` listener pattern as `MermaidBlock`; extract a shared hook if the duplication is trivial to lift

## 3. MarkdownView interception and CSS

- [x] 3.1 In `src/components/MarkdownView.tsx`, add an `svgSource(node)` twin of `mermaidSource(node)` keyed on `language-svg`, branch the existing `<pre>` override to `SvgBlock`, and add `"svg"` to `HIGHLIGHT_OPTIONS.plainText`
- [x] 3.2 In `src/App.css`, add `.svg-block` rules mirroring the `.mermaid-block` contract: block-level centered `<img>`, `max-width: 100%`, `height: auto`, `overflow-x: auto` on the wrapper, and reuse of the error/note styles

## 4. Verification

- [x] 4.1 `bun run build` passes (strict tsc + bundle)
- [x] 4.2 In the running dev app, walk the six spec scenarios with a scratch artifact: valid fence renders as an image; an `xml` fence stays highlighted source; an invalid fence degrades to source with the quiet note while the rest of the artifact renders; a fence containing `<script>`, an `onclick` attribute, and an external `<image href>` shows no execution or network fetch; a naïve fence (no `xmlns`, `viewBox`-only) renders at viewBox size; a `currentColor` fence follows a light/dark scheme toggle while authored colors — including `currentColor` under an author-declared `color` — stay fixed
- [x] 4.3 Confirm no Rust or IPC surface changed (`git diff` touches only `src/` frontend files) and `cargo test` still passes as a workspace sanity check
