# Image Review Design

## Context

See `proposal.md` for motivation. `DiffView` already owns image read/model lifetimes and per-file `ImageMode` state. `ImageWindowRoot` reads the same checked versions independently; `ImageZoom` owns the platform's zoom/pan gestures. Existing Both/Difference controls and CSS blends are duplicated between those surfaces.

Playwright's shared report component offers Diff, Actual, Expected, Side by side, Slider and Blink. Its Diff is a generated attachment; SpecForge has only checked image bytes. Accepted inputs remain limited by the service's existing 8 MiB and 40-million-pixel guards.

## Goals / Non-Goals

**Goals:** Share rendering and decisions between inline and zoom views; preserve read and review lifetimes; keep mode interaction local; bound and cancel expensive highlighting.

**Non-Goals:** Screenshot testing, baseline acceptance, CI image artifacts, image editing, animation-frame comparison, remote image URLs or provider API changes.

## Decisions

### Six image-specific modes independent of text layout

Keep internal `both` as the default but label it Side by side. Replace the old `difference` CSS blend with `diff`, and add `before`, `after`, `slider` and `blink`. Share labels and eligibility through `diffImage.ts`. Two-image modes require two drawable versions, not equal dimensions.

The delta extracts these rules into *Image Comparison Modes*, referenced by the existing *Image Comparison* requirement. Keep that requirement's scenario identities and its read, refusal, safety and zoom contracts intact; the split avoids growing an already oversized legacy requirement rather than suppressing validation.

Reject adding another PR renderer or another global setting: image inspection is view state, and commit detail/zoom must not drift. Reject keeping both Diff and the raw blend: a seventh mode adds little review value.

### One aligned surface for composite modes

A shared `ImageSurface` draws accepted bytes through `ObjectImage` over the union box. Each version uses its own fractional width/height within that box, so unequal versions retain their pixel geometry. Inline views never upscale; zoom supplies the already-scaled union dimensions. Side-by-side frames reserve that same union box.

`decodeSide` applies JPEG EXIF orientation to the service's raw SOF dimensions before deriving any frame or comparison canvas. Only orientations 5–8 swap axes; the browser performs the actual rotation/reflection when displaying or drawing the unchanged bytes. A bounded metadata walk reads the primary TIFF directory inside an EXIF APP1 segment, tolerates missing/malformed metadata and stops before compressed pixels. This preserves pre-decode height reservation and checks the orientation-aware union against the processing ceiling without another image decode or changes to service acceptance.

Inline frames reserve height with a padding pseudo-element rather than relying on CSS `aspect-ratio`; Blink spacing uses an explicit class rather than `:has()`. This keeps the existing Safari 14 support floor. Control line-height resolves through the existing `--leading-tight` token instead of inheriting the image frame's zero line-height, including for 16-pixel icons.

The slider is a focusable ARIA slider on the divider with pointer capture, arrow/Page/Home/End keys and clamped percentage. Its pointer events stop propagation to zoom panning. Blink owns Pause/Resume and a 500 ms timer, disabled while hidden/offscreen; reduced motion starts it paused. Its visual version label is not a repeatedly announced live region.

Reject forcing oriented pixels into unadjusted header-sized boxes and reject automatically blinking outside the explicitly selected mode.

### Bounded, on-demand local highlighting

Use browser-compatible `pixelmatch` in a bundled module worker. Decode checked bytes into top-left-aligned transparent RGBA canvases only after choosing Diff, and enforce a **4,000,000 union-pixel** processing limit before allocation. This is a separate processing bound, never a change to accepted-image preview limits.

Use pixelmatch 8's default OKLab perceptual threshold 0.1 with antialias filtering, muted grayscale unchanged pixels and red highlights. A 0.2 threshold from older screenshot comparators uses a different metric and misses visible green-to-teal asset recolors; a regression test pins that case. Force pixels outside either version's dimensions to highlighted changes so resizing transparent images remains observable. Summaries report pixel count, percentage and dimension changes, not pass/fail.

Keep decoding compatible with WebKit using object-URL `Image` and Canvas on the main thread; send transferable pixel buffers to the worker for comparison, then paint returned pixels onto a canvas. Revoke temporary URLs, terminate workers on cancellation, disregard obsolete async replies and show a local fallback if decoding, canvas access or worker construction fails. No persistent pixel cache.

Reject full-size unbounded canvas work and synchronous pixelmatch on the UI thread. Reject service-side decoding/new IPC because this is a presentation artifact, not a domain read. Reject requiring OffscreenCanvas/createImageBitmap in workers because those capabilities vary across desktop webviews.

```mermaid
sequenceDiagram
    participant View as Shared image surface
    participant Decode as Checked bytes / local canvas
    participant Worker as Pixel worker
    View->>Decode: Choose Diff; check union pixel ceiling
    Decode-->>View: RGBA buffers
    View->>Worker: Transfer buffers and dimensions
    Worker-->>View: Highlight buffer and changed count
    View->>View: Paint canvas; show summary
    Note over View,Worker: Cleanup cancels obsolete work and releases resources
```

### Preserve zoom geometry and safety

Extend `ZoomFrame` with shared comparison content rather than duplicating slider/blink/diff rendering. Keep the first viewport element stable across mode switches. Retain the intended scroll position separately from the actual offsets each viewport accepts: switching to one wider frame may clamp those offsets, but returning to two frames restores the intended position. Programmatic/clamped scroll events do not replace that position; a subsequent reader pan or zoom does. Restore returning viewports together after layout. Keep existing pinch, scroll, keyboard zoom, pixelated magnification and object-URL/CSP restrictions.

Reject handing generated bytes through window URLs or changing the window-opening API.

## Risks / Trade-offs

- Additional local memory → cap union work at four million pixels; release canvases, URLs and workers at cleanup.
- Animated images could yield frame-dependent pixels → Diff compares the locally decoded frame, not an animation timeline; retain original version viewing.
- Hidden retained PR sections could keep timers → intersection/document visibility checks and timer cleanup; no live-region announcements on each blink.
- Platform decoding/worker failures → keep the rest of the viewer usable and report a local comparison failure.
- Changed image layout semantics → explicitly update the image and text-layout contracts; text layout, budgets and review state remain untouched.
- Diff colors represent image data, not theme identity → fixed red/muted output pixels; surrounding controls and checkerboards use existing theme tokens.
