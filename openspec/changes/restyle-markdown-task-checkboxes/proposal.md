# Restyle Markdown Task Checkboxes

## Why

Task checkboxes in the rendered markdown view are the last raw OS control in the content area: a disabled native macOS checkbox that WKWebView renders washed-out and gray, undersized (~14px) against the 16px Inter body text. Every other markdown element got the visual-identity treatment (blockquote cards, unboxed inline code, the type pass) — the checkbox never did, so completed tasks read as dim noise instead of a confident "done" mark.

## What Changes

- Replace the native `<input type="checkbox">` rendering in the markdown view with custom SVG glyphs drawn from the app's icon vocabulary, sized to sit flush with the body text (~16px).
- Checked state reads as **status, not control**: a solid `--ok-strong` rounded square with the check knocked out in `--bg` (the plane the markdown surfaces sit on) — the squared-off sibling of the tree's `CompletionMark` — rather than an accent-filled control. This keeps the accent-fill discipline intact (accent stays filled in exactly three places). `--ok-strong` is the established AA-clearing foreground "done" green; raw `--ok` stays reserved as the progress-meter fill.
- Unchecked state renders as a quiet outlined box in `--border-strong` with no fill.
- A checked task's line text dims to `--text-faint` (no strikethrough — the glyph already carries the "done" signal), so pending work jumps out when scanning `tasks.md`.
- Checkbox glyphs remain inert and expose checkbox semantics to assistive technology (checked/unchecked state still announced, read-only).
- The settings-view toggle is untouched — it is a real interactive control and keeps its native accent-color rendering.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `visual-identity`: adds a markdown task-checkbox treatment requirement (glyph construction, `--ok-strong` status coloring, 16px sizing, checked-line text dimming); clarifies that the checkbox `accent-color` mention in the Accent Color requirement applies to the settings toggle only; extends the Outlined Chip Badges fill sanction so the checked markdown checkbox is recorded as the document-surface sibling of the completion mark (a sanctioned, glow-free `--ok-strong` done fill); and rewords the Markdown Body requirement's out-of-scope sentence change-agnostically so it no longer reads as prohibiting the checkbox treatment its sibling requirement mandates.

## Impact

- **Frontend only** — no Rust, IPC, or parser changes.
- `src/components/MarkdownView.tsx`: the existing `input[type=checkbox]` component override swaps the native input for the glyph rendering (with accessible checkbox semantics).
- `src/components/icons.tsx`: a filled task-checkbox glyph (CompletionMark's disc-with-knockout construction, squared); the idle `Square` outline icon covers the unchecked state or is superseded by a matching new glyph.
- `src/App.css`: `.markdown-view` task-list rules — glyph sizing/alignment, `--ok-strong` / `--border-strong` coloring, `--text-faint` dimming on checked task lines.
- `openspec/specs/visual-identity/spec.md`: delta spec for the new requirement and the Accent Color clarification.
- `spec-browser`'s Read-Only Viewer requirement is unaffected: static glyphs satisfy "clicking a rendered checkbox does not modify the underlying file" trivially.
