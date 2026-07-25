# Design — Restyle Markdown Task Checkboxes

## Context

The markdown view (`src/components/MarkdownView.tsx`) intercepts GFM task-list checkboxes and renders them as `disabled readOnly` native `<input type="checkbox">` elements, styled only with `accent-color: var(--accent)` (`.markdown-view li.task-list-item > input`, `src/App.css`). WKWebView renders disabled controls washed-out and gray, and the native glyph is ~14px against 16px Inter body text — the last raw OS control in a content area that otherwise went through the visual-identity pass.

The app already has a "done" vocabulary to match:

- `CompletionMark` (`src/components/icons.tsx`) — a solid disc with a knocked-out check, two-class construction (`.completion-mark-disc` fill `--ok-strong`, `.completion-mark-check` stroke `--surface`, stroke-width 2.5, round caps/joins).
- `--ok-strong` — the foreground "done" green established by `tint-completed-change-rows` (`#047857` light / `#34d399` dark, AA-clearing). `--ok` (`#10b981` / `#34d399`) is reserved as the progress-meter fill and the source hue for the wash tints; it is *not* a foreground token (only ~2.6:1 on light `--surface`).
- `--border-strong` — the load-bearing control edge.
- Idle icons `Square` / `CheckSquare` (24×24 viewBox, `currentColor`, 1.5 stroke via the shared `Svg` wrapper) that were added with the icon set but never wired up.

Every surface that renders `.markdown-view` (detail pane via `.split-pane-right`, archive reading view, file-browser preview column) sits on `background: var(--bg)` with no local override.

Decided in exploration with the user: the checkbox is a **status, not a control** (`--ok` family, not accent — the accent-fill discipline stays at exactly three places); checked = filled square with knocked-out check (CompletionMark's construction, squared); unchecked = quiet outlined box; size ≈16px flush with body text; checked lines dim to `--text-faint` with **no** strikethrough.

## Goals / Non-Goals

**Goals:**

- Replace the native markdown checkbox with SVG glyphs from the app's icon vocabulary, in both states, at ~16px.
- Checked state reads as a confident "done" mark: solid `--ok-strong` rounded square, check knocked out to the pane's plane (`--bg`).
- Unchecked state reads as a quiet pending box: outline in `--border-strong`, no fill.
- Checked task lines recede to `--text-faint` so pending work jumps out when scanning `tasks.md`.
- Preserve read-only behaviour (spec-browser → Read-Only Viewer) and expose checkbox state to assistive technology.
- Apply uniformly wherever `.markdown-view` renders (detail pane, archive reading view, file-browser preview).

**Non-Goals:**

- No interactivity — checkbox toggling / write-back stays out (v1 is read-only; `SelfWriteTracker` remains idle).
- No changes to the settings toggle (a real control; keeps native rendering and `accent-color`).
- No changes to the sidebar tree's task rows (spec-browser forbids leading glyphs there; its green-struck-label grammar stands).
- No broader markdown-rendering changes (callouts, anchors, code-block chrome remain out of scope, as before).

## Decisions

### 1. SVG glyph components, not a CSS restyle of the native input

Replace the `<input>` in `MarkdownView`'s existing `input` component override with glyph rendering. The alternative — `appearance: none` on the input and drawing box + check in CSS — fights WebKit: pseudo-elements don't render on `<input>`, so the check glyph would need `mask`/data-URI tricks where CSS tokens can't reach (data URIs can't read custom properties). The icon vocabulary already exists and colors flow naturally through classes.

### 2. New `TaskCheckMark` icon for checked; reuse `Square` for unchecked

- **Checked:** a new `TaskCheckMark` in `icons.tsx` — filled rounded square + knocked-out check, mirroring `CompletionMark`'s two-class construction (it cannot use the `Svg` wrapper: the body is a fill, not a `currentColor` stroke, and its two parts take different tokens). Geometry in the 24×24 viewBox: `<rect x="3" y="3" width="18" height="18" rx="4.5">` (matching the disc's r=10 coverage) and the check polyline `7.5 12.5 10.5 15 16.5 8.5` — identical points, stroke-width 2.5, round caps/joins as `.completion-mark-check`, so the two marks read as siblings.
- **Unchecked:** the existing, currently-unused `Square` icon (outline rect, `currentColor` via the `Svg` wrapper); CSS colors it `--border-strong`.

Alternative considered: reusing `CheckSquare` (outline box + check) for the checked state — rejected in exploration; the outline construction lacks the visual weight that motivated this change, and the filled construction rhymes with the tree's completion disc.

### 3. Colors: `--ok-strong` fill, `--bg` knockout, `--border-strong` outline

- Checked fill is `--ok-strong`, **not** `--ok`: `tint-completed-change-rows` established `--ok-strong` as the AA-clearing foreground "done" green (the completion disc's fill), while `--ok` stays the meter fill. A `--ok` fill would be ~2.6:1 against light surfaces.
- The knocked-out check strokes in `--bg` — the plane every `.markdown-view` surface actually sits on (`CompletionMark` uses `--surface` because the sidebar is a `--surface` plane; the markdown pane is not).
- Unchecked outline is `--border-strong` (the load-bearing control edge), not `--border` (decorative hairline, explicitly never the sole signal of a control boundary).
- No glow: `--glow-ok` stays reserved for the in-progress meter.

### 4. 16px rendered size, aligned via CSS

Glyphs render at `width={16} height={16}` (24 viewBox scales down; the 2.5 check stroke lands at ~1.67px, visually matching the tree's 15px completion mark). Alignment/spacing live in `.markdown-view` CSS: an `inline-flex` glyph slot (the span centers the SVG it wraps), small `translateY` baseline nudge (tuned visually against `--text-lg` at `--leading-prose`), ~0.5em right margin. The existing `li.task-list-item` negative-margin/padding layout is adjusted for the wider glyph.

### 5. Checked-line dimming via a class set in the existing `li` override

`MarkdownView` already overrides `li` (to stamp `data-line`). Extend it to detect a checked task checkbox among the li's children in hast — checking direct children **and** one level into a leading `<p>` (GFM loose lists wrap the input in a paragraph) — and append a `task-list-item--done` class. CSS then sets `color: var(--text-faint)` on that class; the color cascades to nested content by design (sub-bullets of a done task recede with it), with one reset rule — `.markdown-view li.task-list-item--done li.task-list-item:not(.task-list-item--done) { color: var(--text) }` — so a *pending* subtask under a completed parent never dims.

Alternative considered: pure CSS `:has()`. Supported in current WKWebView, but the hast-side class keeps the state decision in one place (the same component that renders the glyph), costs ~6 lines, and leaves the stylesheet free of support caveats.

No strikethrough: the glyph is the "done" signal here. (The tree strikes its completed task labels because rows there carry no glyph — different surface, same green.)

### 6. Accessibility: `role="checkbox"` + `aria-checked` + `aria-disabled` on a wrapping span

The glyph pair renders inside `<span role="checkbox" aria-checked={checked} aria-disabled="true" class="task-checkbox …">`, with the SVG itself `aria-hidden`. Screen readers reading the line announce checked/unchecked state; `aria-disabled` communicates non-operability; no `tabIndex`, so the document doesn't grow a focus stop per task.

Alternative considered: keeping a visually-hidden real `<input disabled>` behind the glyph (GitHub's approach). More DOM per task line for no behavioural gain in a fully read-only document; rejected.

Non-checkbox `<input>`s in markdown (vanishingly rare) keep the existing native fallback branch.

### 7. CSS cleanup

The `.markdown-view li.task-list-item > input[type="checkbox"]` rule (accent-color, translateY) is removed with the native input; the visual-identity spec's Accent Color requirement gets a delta clarifying that its "checkbox `accent-color`" applies to the settings toggle only.

## Risks / Trade-offs

- **[Baseline alignment varies by context]** (task lines in blockquotes, nested lists) → em-relative spacing plus a fixed translateY tuned visually in the running app (`bun run wt:dev`); the glyph is verified against `tasks.md`, a loose-list document, and a nested checklist.
- **[hast shape drift]** — detection assumes remark-gfm's `li > input` / `li > p > input` shapes → detection is written against both; if neither matches, the li simply gets no `--done` class (graceful: glyph still renders, text keeps normal color).
- **[Dimming cascades into nested pending tasks]** → explicit reset rule scoped to `li.task-list-item:not(.task-list-item--done)`.
- **[Screen-reader semantics of a non-focusable role="checkbox"]** — some AT behaviours differ from a real disabled input → state is still announced during linear reading, which is the only interaction mode this read-only document supports; revisit if checkbox toggling ever lands (then a real input is required anyway).
- **[Contrast of the knockout check]** — `--bg` on `--ok-strong` in both schemes: dark `#0a0d12` on `#34d399` and light `#ffffff`-family `--bg` on `#047857` both clear non-text 3:1 comfortably.

## Open Questions

None blocking. The exact `translateY` nudge and right-margin are tuned visually during implementation (the spec pins the 16px size and token choices, not sub-pixel alignment).
