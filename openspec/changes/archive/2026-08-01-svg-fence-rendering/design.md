# Design: SVG Fence Rendering

## Context

The rich frontend renders artifact markdown through `react-markdown` + `remark-gfm` + `rehype-highlight` with component overrides in `src/components/MarkdownView.tsx`. One fence family is already special-cased: the `<pre>` override calls `mermaidSource(node)` and, when the fence's info string is `mermaid`, hands the reconstructed source to `MermaidBlock` instead of the code well. `HIGHLIGHT_OPTIONS.plainText = ["mermaid"]` keeps rehype-highlight from shredding that fence into token spans before the override reads it.

Today the three ways SVG could appear in an artifact all fail differently: raw `<svg>` HTML is silently dropped (no `rehype-raw`), an `svg` fence renders as highlighted XML source, and `![…](x.svg)` file references resolve against the app origin and break. This change addresses exactly one of the three: the `svg` fence.

`MermaidBlock.tsx` establishes the contract this component mirrors: graceful degradation to source with a quiet note, design-token theming with a `prefers-color-scheme` listener that re-keys the render, and a strict no-active-content posture. The CSS contract in `src/App.css` (`.mermaid-block`: centered child, `max-width: 100%`, `height: auto`, `overflow-x: auto`) transfers directly.

The corresponding requirement lives in the `spec-browser` capability ("Mermaid Diagram Rendering", `openspec/specs/spec-browser/spec.md`); this change adds a sibling requirement, not a new capability.

## Goals / Non-Goals

**Goals:**

- Render ` ```svg ` fences as images in the rich frontend, self-contained in the artifact text.
- Structural security: fence content can never execute scripts or load external resources — parity with the mermaid requirement's "no active content" scenario without adding a sanitizer.
- Naïve fences Just Work: authors (usually agents) who omit `xmlns` or size attributes still get a correct render.
- An opt-in theming convention (`currentColor`) that follows the light/dark scheme, re-rendering on scheme change.
- Graceful degradation to source for invalid fences, visually identical to the mermaid fallback.

**Non-Goals:**

- File-referenced images (`![…](./x.svg)`) — different plumbing (base-dir resolution across three call sites, a bytes command for rasters, a remote-URL policy); deliberately a separate future change.
- Raw inline `<svg>`/HTML in markdown (`rehype-raw` + sanitization) — rejected: largest attack surface, changes semantics of every artifact, no GitHub parity.
- Content-sniffing other fences (`xml` etc.) — only the explicit `svg` info string is intercepted.
- Repainting authored colors for dark mode (matte cards, inversion filters) — the renderer must not alter colors it didn't author; only `currentColor` is substituted.
- `terminal-ui` rendering — it cannot render SVG and continues to show fences as code text.

## Decisions

### D1: Render through an inert `<img>` with a data URI — not inline DOM

**Chosen:** serialize the (rewritten) SVG document to `data:image/svg+xml,…` (URI-encoded) and set it as the `src` of an `<img>`.

**Why:** in an image context, scripts, event handlers, and external fetches are dead *by construction* — the security scenario is satisfied structurally rather than by configuration or a sanitizer library. No new dependency (DOMPurify), no `dangerouslySetInnerHTML`, nothing to audit. SMIL/CSS animations still run (acceptable, even pleasant); `foreignObject` renders but stays inert.

**Alternatives considered:**
- *Inline SVG + DOMPurify*: enables text selection and native `currentColor` inheritance, but adds a dependency, an XSS audit surface, and `dangerouslySetInnerHTML`. Rejected — artifact content is often agent-authored, and the img context gives the same theming result via the root `color` injection (D4).
- *Blob URL instead of data URI*: equivalent security; adds lifecycle management (revocation) for no benefit at realistic fence sizes. Noted as the escape hatch if a data-URI length limit is ever hit in practice.

### D2: Intercept at the existing `<pre>` override; `svg` stays on the highlighted path

`fenceSource(node, language)` — one parameterized child walk, replacing the earlier separate `mermaidSource`/`svgSource` twins — backs both interceptions. Only `"mermaid"` joins `HIGHLIGHT_OPTIONS.plainText`; `svg` deliberately does not, and that asymmetry is the point:

- **`mermaid` must be exempted.** Every ` ```mermaid ` fence is intercepted unconditionally — there is no validity gate at the `MarkdownView` level that can decline and fall through to the ordinary code path; invalid diagram source is handled inside `MermaidBlock` itself, which always shows raw (unhighlighted) source on failure. The fence is never meant to be displayed highlighted, so nothing is lost by exempting it.
- **`svg` must NOT be exempted.** Unlike `mermaid`, `SvgBlock`'s validity gate (D3) commonly declines — a fence someone happened to mark `svg` but whose body isn't a well-formed, root-`<svg>` document must still read as an ordinary syntax-highlighted code block (hljs aliases `svg` to its `xml` grammar), not as unhighlighted plain text behind an alarming error banner. Leaving `svg` on the normal highlighting path and handing `SvgBlock` the already-highlighted `<pre>` as a `fallback` prop (see D3) gets this for free: rehype-highlight still shreds the fence into `hljs-*` token spans, but `fenceSource`'s `textOf`-based reconstruction walks any element tree — span-shredded or not — and returns the same concatenated text either way, so `SvgBlock`'s own parse of the source is unaffected by whether `svg` is exempted.

### D3: `DOMParser("image/svg+xml")` is both the validity gate and the rewrite substrate

The fence body is parsed once. A `parsererror` document — or any other gate failure — routes to the fallback UI: a quiet note over the fence's own already-syntax-highlighted `<pre>`, passed down from `MarkdownView` as a `fallback` prop rather than rebuilt from raw source (D2), so an invalid `svg` fence reads as an ordinary code block. The presentation (`fence-block--error` / `fence-block__note`) is the same shared CSS `MermaidBlock` uses. A valid document is rewritten in place (D4) and serialized with `XMLSerializer`. There is no async step at all — no lazy chunk, no loading state, no attempt counter; the whole pipeline is a synchronous `useMemo`, re-keyed by source and color scheme.

One subtlety: a missing `xmlns` is **not** an XML well-formedness error — `DOMParser` succeeds on a namespace-less `<svg>` body (verified empirically in WKWebView during review), returning a root with localName `svg` and a null namespace. The parser alone therefore provides no SVG-ness discrimination at all: any well-formed XML (`<foo/>`) parses just as cleanly. The gate is accordingly two checks: (1) the parse produced no `parsererror` document, and (2) the root element's localName is `svg`, in either the SVG namespace or no namespace. A null namespace is what triggers the `xmlns` injection of D4 — the declaration is required for a *standalone* SVG document yet routinely omitted by authors who learned SVG inline in HTML, where the HTML parser auto-namespaces. Only the image load itself fails without `xmlns`; an `onError` handler on the `<img>` is the second net for anything the gate mispredicts.

### D4: A three-part rewrite pass on the parsed document

1. **`xmlns` injection** when absent (see D3) — without it the image paints nothing. Mechanism (measured in WKWebView during verification): a parsed tree cannot be re-namespaced in place, and a declaration lookalike added via `setAttribute("xmlns", …)` serializes to a document the `<img>` loads *successfully* but paints as nothing (so even the `onError` net misses it) — therefore the injection patches the fence text and re-parses, yielding a tree whose every element is genuinely in the SVG namespace.
2. **Deterministic sizing**: when the root lacks usable absolute `width` AND lacks usable absolute `height` — both, not just one — but declares a `viewBox`, set `width`/`height` from the viewBox extents at one user unit per CSS pixel. When exactly one of `width`/`height` is authored and usable, both are left untouched: the image context derives the missing dimension from the viewBox ratio on its own, so overwriting the authored one would silently discard it. Rationale: an SVG with only a `viewBox` has an intrinsic *ratio* but no intrinsic *size* in an image context, so the WebView falls back to arbitrary replaced-element defaults (~300px). CSS (`max-width: 100%`, `height: auto`, mirroring `.mermaid-block > svg`) caps oversized results while preserving ratio. A fence with neither `viewBox` nor dimensions is degenerate; browser defaults are accepted rather than specified.
3. **Theme `color` injection** — when the root `svg` element does not already declare a `color` (attribute or inline style), set `color: <live --text token>` on it (token read off `:root`, the same pattern `MermaidBlock.themeVariables()` uses). Every `currentColor` occurrence then resolves to the token through ordinary CSS inheritance *inside* the image document, while an author-declared `color` — on the root or any descendant subtree — wins naturally. No occurrence rewriting takes place, so there is no presentation-attribute inventory to maintain, no `<style>` CSS text to string-edit, and no risk of touching text nodes that happen to contain the word "currentColor".

Additionally, a root-level `<title>` element's text becomes the `<img>` `alt`; otherwise a generic label is used.

### D5: Theming is opt-in via `currentColor`; the renderer never repaints authored colors

In an image context the SVG document is isolated: host CSS custom properties do not cascade in (`var(--text)` in author SVG can never resolve), and the host page's `color` does not inherit across the boundary either. The `color` cascade *within* the image document works normally, though — which is exactly what D4 leans on: injecting the token as the root `color` themes every `currentColor` site by plain inheritance, and `currentColor` stays the documented convention because it is the only theming hook that survives the boundary. A `prefers-color-scheme` listener (same `DARK_SCHEME` pattern as `MermaidBlock`; extract a small shared hook if it stays duplicated) re-keys the memo so the injected token follows scheme changes live.

The honest limit, accepted deliberately: SVG's default fill is black, so a fence that uses no colors at all stays near-invisible on the dark surface. Authors opt in by using `currentColor`; the renderer does not "fix" anything else (no matte, no inversion) because that would repaint deliberate artwork. Revisit only with evidence from real artifacts.

## Risks / Trade-offs

- **[WKWebView sizing assumptions]** The viewBox-only intrinsic-size fallback (~300px default object size) is standard replaced-element behavior but was reasoned, not measured, in WKWebView → tasks include an early verification spike in the dev app with a scratch artifact covering all three sizing cases before the rewrite pass is finalized.
- **[Root-name gate is a heuristic]** Well-formed non-SVG XML is rejected by the root-localName check, not by the parser (which accepts any well-formed XML); a document whose root happens to be named `svg` outside the SVG namespace would still slip to the `<img>` stage → the `onError` fallback shows source, identical UX to the parse-gate fallback.
- **[Data-URI length ceilings]** Very large fences could exceed engine URI limits → limits are multi-MB in WebKit; a fence that large is an authoring smell, and a Blob URL is a drop-in escape hatch (D1).
- **[GitHub degradation is worse than mermaid's]** GitHub renders mermaid fences natively but shows `svg` fences as code blocks → accepted; the fence body is still readable source, and self-containment (no side files surviving archive moves) is the compensating benefit.

## Open Questions

_None blocking. The only empirical unknown (WKWebView sizing) is scheduled as the first implementation task rather than left open here._
