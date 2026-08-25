# Tasks — Maximize Diagrams and SVG Figures

## 1. Pure Zoom Module

- [ ] 1.1 Create `src/components/figureZoom.ts` with the module's value types — a `ZoomState` carrying scale and both scroll offsets, and a `Extents` pair for viewport and content dimensions. Keep every export a total function of its arguments with no DOM access, so `bun test` can reach all of it (`design.md`: *Decision 4*)
- [ ] 1.2 Implement `fitScale(viewport, content, padding)` returning $\min\left(\frac{W_v - 2p}{W_c}, \frac{H_v - 2p}{H_c}\right)$, so the figure opens fully visible on whichever axis constrains it (`spec-browser`: *Maximized Figure View*)
- [ ] 1.3 Implement `clampScale(scale, fitScale)` bounding scale to $[\min(s_{\text{fit}}, 1),\ s_{\max}]$ with $s_{\max} = 8$, so the figure can neither be reduced past fit nor enlarged without limit (`spec-browser`: *Maximized Figure View*)
- [ ] 1.4 Implement `zoomAt(state, factor, pointer, viewport, content)` returning the new scale and both scroll offsets from $\ell' = \frac{s'}{s}(\ell + c) - c$, applied per axis, so the point under the pointer stays under it (`spec-browser`: *Maximized Figure View*)
- [ ] 1.5 Implement `pinchFactor(previous, current)` as the ratio of two-contact separations, returning a neutral factor when either separation is degenerate so a pinch beginning with coincident contacts cannot produce a non-finite scale (`spec-browser`: *Maximized Figure View*)
- [ ] 1.6 Implement `panBy(state, delta, viewport, content)` clamping both scroll offsets to their valid ranges, so a drag cannot push the figure entirely out of the surface
- [ ] 1.7 Write `src/components/figureZoom.test.ts` covering: fit constrained by width and by height separately; clamp at both bounds and at the fit-below-one case; `zoomAt` holding the anchor point at the viewport's left edge, centre, and right edge; `zoomAt` composed with its inverse returning to the original offsets; `pinchFactor` on a degenerate separation; and `panBy` clamping at all four limits (`design.md`: *Risks / Trade-offs* — these tests are the change's only automated gate)

## 2. Lightbox Surface

- [ ] 2.1 Create `src/components/FigureLightbox.tsx` rendering a native `<dialog>` through `createPortal`, opened with `showModal()` on mount and closed with `close()`, taking the figure as `children` plus its natural extents (`design.md`: *Decision 6*)
- [ ] 2.2 Add the dismissal paths — an explicit close control, a click on the backdrop outside the figure, and Escape — each returning to the artifact with its scroll position untouched (`spec-browser`: *Maximized Figure View*)
- [ ] 2.3 Call `preventDefault()` on the lightbox's own Escape keydown so `App.tsx`'s outermost document handler stands down and a Settings or Archive pane open behind it stays open, matching the `defaultPrevented` contract the Settings rename input already uses (`spec-browser`: *Maximized Figure View*, `design.md`: *Decision 6*)
- [ ] 2.4 Wire wheel zoom and drag pan through `figureZoom.ts`, setting the figure's layout width from the scale and driving pan through the container's `scrollLeft` / `scrollTop` — never through a CSS transform, so the image path re-rasterizes instead of magnifying a fixed raster (`spec-browser`: *Maximized Figure View* — fidelity, `design.md`: *Decision 3*)
- [ ] 2.5 Implement two-contact pinch zoom over pointer events, tracking active contacts by `pointerId` so a mouse, a touch contact, and a pen all drive pan and zoom through the same path (`touch-input`: *Drag Interactions Accept Pointer Input*)
- [ ] 2.6 Settle the `touch-action` question on a real touch device before finalising 2.5: prototype native container panning against fully manual panning, and adopt whichever reliably lets a pinch start (`design.md`: *Risks / Trade-offs* — this is the one open behavioural question)
- [ ] 2.7 Add the fit and actual-size controls, plus zoom-in and zoom-out controls so the view is fully operable without a wheel or a trackpad (`spec-browser`: *Maximized Figure View*, `touch-input`: *Interactive Targets Meet a Minimum Size on Coarse Pointers*)
- [ ] 2.8 Preserve scale and scroll offsets across a change to the rendered figure, so a colour-scheme flip or a live file edit re-renders in place without resetting the reader's position (`spec-browser`: *Maximized Figure View*)

## 3. Fence-Block Integration

- [ ] 3.1 Add the maximize affordance and local `maximized` state to `src/components/MermaidBlock.tsx`, rendering `FigureLightbox` with the same `svg` string the inline block holds — not a copy — so a scheme-driven re-render flows through to the maximized figure (`design.md`: *Decision 1*)
- [ ] 3.2 Gate that affordance on a successful render: absent while `svg === null` (pending) and absent on the `fence-block--error` fallback, since neither has a figure to maximize (`spec-browser`: *Maximized Figure View*)
- [ ] 3.3 Add the same affordance and state to `src/components/SvgBlock.tsx`, passing the existing `data:` URI to the lightbox so the maximized image is still presented through an image context and the fence body is still never injected into the live DOM (`spec-browser`: *SVG Fence Rendering*, *Maximized Figure View* — security posture)
- [ ] 3.4 Gate the `SvgBlock` affordance on `rendered !== null && rendered.src !== erroredSrc`, so a fence that failed its parse gate or whose `<img>` failed to load offers no maximized view (`spec-browser`: *Maximized Figure View*)
- [ ] 3.5 Render each affordance as a real `<button>` with an accessible label naming what it opens, so it is reachable and activatable by keyboard without any new key handling (`spec-browser`: *Shell Keyboard Operability*)
- [ ] 3.6 Confirm `src/components/MarkdownView.tsx` gains no new props in the course of 3.1–3.5; the memo's shallow comparison depends on every prop staying a primitive or a stably-identified ref (`design.md`: *Decision 1*)

## 4. Reconciliation and Styling

- [ ] 4.1 Key the `MarkdownView` element at `src/components/DetailPane.tsx` on the render target's artifact identity, so navigating to a different artifact remounts the subtree and cannot reuse a fence component that still holds a maximized state (`spec-browser`: *Maximized Figure View* — navigating away, `design.md`: *Decision 7*)
- [ ] 4.2 Verify the same keying leaves a same-artifact content edit reusing the subtree, so a watcher-driven reparse updates the maximized figure in place rather than closing it (`spec-browser`: *Maximized Figure View*, *Reactive Updates from Filesystem*)
- [ ] 4.3 Style the lightbox surface and its `::backdrop` in `src/App.css` from the design tokens, giving the figure a padded content area consistent with $p$ in the fit formula (`visual-identity`: *Design Token Layer*)
- [ ] 4.4 Style the maximize affordance to match the repository's existing hover-revealed treatment, rendered visibly at rest under `@media (hover: none)` and given an enlarged hit area under `@media (pointer: coarse)`, mirroring how the favorite toggle and the pane chevrons already satisfy these rules (`touch-input`: *Essential Controls Are Discoverable Without Hover*, *Interactive Targets Meet a Minimum Size on Coarse Pointers*)
- [ ] 4.5 Confirm the inline `max-width: 100%` on `.markdown-view .mermaid-block > svg` and `.markdown-view .svg-block > img` is untouched, so the inline reading default is unchanged (`design.md`: *Decision 5*)

## 5. Verification

- [ ] 5.1 Run `bun test` and confirm the `figureZoom` suite passes alongside the existing suites
- [ ] 5.2 Run `bun run build` — strict `tsc --noEmit` with `noUnusedLocals` / `noUnusedParameters`, then the bundle — and confirm it is clean
- [ ] 5.3 Run `bun run build` once in this worktree before `cargo test`, then run `cargo test` to confirm no Rust regression; the workspace suite fails workspace-wide in a fresh worktree until `dist/` exists, and the failure surfaces as an opaque proc-macro error rather than a missing bundle
- [ ] 5.4 Smoke the change in the running app — `bun run wt:dev` for this worktree's slot — walking the *Maximized Figure View* scenarios: maximize a wide diagram and confirm its labels become readable; zoom with the pointer over a specific node and confirm that node stays under the pointer; zoom out past fit and in past the ceiling and confirm both bounds hold; drag to pan an enlarged figure; maximize an `svg` fence and enlarge it well beyond its inline size, confirming it stays sharp rather than pixelating
- [ ] 5.5 Smoke the dismissal and scope rules in the same session: open the Archive view, maximize a figure in the artifact behind it, and confirm one Escape closes only the lightbox and a second closes the Archive; confirm the URL and Back/Forward history are unchanged throughout; navigate to a different artifact while maximized and confirm the view closes
- [ ] 5.6 Smoke the live-update and scheme paths: with a figure maximized and enlarged, switch the operating system between light and dark and confirm the figure re-renders in the active scheme with its scale and position preserved; edit the artifact's fence on disk and confirm the maximized figure follows the new source without closing
- [ ] 5.7 Confirm the negative cases in the app: a `mermaid` fence with invalid source and an `svg` fence with a malformed body each show their source fallback with no maximize control, and a fence in another language is unaffected
- [ ] 5.8 Repeat 5.4 in a browser against `specforge-serve` to confirm parity between the desktop shell and the served web UI, since both frontends share one bundle (`web-ui`: *Read-Only Parity With the Desktop Frontend*)
