# Tasks — Restyle Markdown Task Checkboxes

## 1. Icon

- [x] 1.1 Add `TaskCheckMark` to `src/components/icons.tsx`: filled rounded square (`rect x=3 y=3 width=18 height=18 rx=4.5`) plus the completion-mark check polyline (`7.5 12.5 10.5 15 16.5 8.5`), two-class construction (`task-check-mark-box` / `task-check-mark-check`) without the `Svg` wrapper, mirroring `CompletionMark`'s doc-comment style (explain why it bypasses the wrapper and where its colours resolve)

## 2. MarkdownView rendering

- [x] 2.1 Replace the checkbox branch of the `input` component override in `src/components/MarkdownView.tsx`: render `<span role="checkbox" aria-checked aria-disabled="true" className="task-checkbox">` wrapping `TaskCheckMark` (checked) or `Square` (unchecked) at 16px, SVG `aria-hidden`; keep the native fallback branch for non-checkbox inputs
- [x] 2.2 Extend the existing `li` override to detect a checked task checkbox among the li's hast children — direct children and one level into a leading `<p>` (loose lists) — and append `task-list-item--done` to the class list when found

## 3. Stylesheet

- [x] 3.1 Replace the `.markdown-view li.task-list-item > input[type="checkbox"]` rule in `src/App.css` with `.task-checkbox` rules: inline-flex glyph slot, 16px sizing, baseline `translateY` nudge, 0.5em right margin, and a hanging indent (`margin-left: calc(-16px - 0.5em)` hangs the glyph in the list gutter so wrapped task text aligns on the content edge); remove the inert `li.task-list-item` negative-margin/padding pair
- [x] 3.2 Colour the glyphs by token: `task-check-mark-box` fill `--ok-strong`, `task-check-mark-check` stroke `--bg` (stroke-width 2.5, round caps/joins, matching `.completion-mark-check`), unchecked `Square` stroked `--text-faint` (clears the 3:1 non-text floor in both schemes; light `--border-strong` does not); no glow anywhere
- [x] 3.3 Add the checked-line dimming rules: `.markdown-view li.task-list-item--done { color: var(--text-faint) }` with `code { color: inherit }` inside done lines and the nested-pending reset `.markdown-view li.task-list-item--done li.task-list-item:not(.task-list-item--done) { color: var(--text) }`; no line-through anywhere; darken light `--text-faint` to `#687380` so 16px done prose clears AA

## 4. Verification

- [x] 4.1 `bun run build` passes (strict tsc + vite bundle)
- [x] 4.2 Run the app and visually verify against a real `tasks.md` in both colour schemes: 16px glyphs flush with body text, checked = `--ok-strong` square with `--bg` knockout, unchecked = `--text-faint` outline, checked lines dimmed to `--text-faint` with no strikethrough, pending subtask under a checked parent stays `--text` (verified via the dev server's rendering in both schemes; the light pass surfaced and fixed the pending-outline and faint-AA contrast issues)
- [x] 4.3 Verify a loose-list task document (blank lines between items) still gets glyphs and dimming (the `li > p > input` hast shape) — verified visually and in the DOM; the archive reading view + file-browser preview share the same `.markdown-view` renderer with no background overrides (verified in the stylesheet), so the treatment is structurally identical there
- [x] 4.4 Inspect the DOM: `role="checkbox"` / `aria-checked` / `aria-disabled="true"` present on every glyph, none keyboard-focusable, SVGs `aria-hidden`, zero native task `<input>`s, `data-line` scroll anchors intact; settings toggle untouched (its `accent-color` rule survives at exactly one site, outside `.markdown-view`)
