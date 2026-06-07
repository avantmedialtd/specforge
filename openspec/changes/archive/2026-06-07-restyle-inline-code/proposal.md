# Restyle Inline Code

## Why

Inline `<code>` in the markdown detail pane has been an outlined transparent chip — a 1px `--border-strong` box with neutral `--text` glyphs. In dense spec prose (the `visual-identity` spec alone has 103 inline-code spans) a low-contrast hairline box around 9px mono text barely separates from the surrounding 16px Inter, so code doesn't *pop* when scanning a paragraph. The treatment had been ratcheted from a faint hairline up to `--border-strong` (commit `0b2edb0`) precisely because the "everything outlined, nothing filled" doctrine left only border-weight as a contrast lever — the weakest one — and it had hit its ceiling.

The strongest figure/ground cue available without introducing a fill (which the row-grammar doctrine reserves for the progress meter) is **colour on the glyphs themselves**. Colouring inline code and dropping the box reads clearly against prose, matches the familiar "tinted inline code" convention from editors and rendered-markdown tools, and — by using a non-accent hue — keeps code visually separate from the accent-coloured markdown links.

## What Changes

- Inline `<code>` (not inside `<pre>`) becomes **unboxed colour-tinted text**: no background fill, no border, no border-radius, no chip padding. It is distinguished from body prose by `--font-mono`, `font-weight: 500`, and a dedicated `--code-fg` colour token.
- A new `--code-fg` token is added per scheme: a darker emerald on light (`#047857`, ~5.3:1 on the near-white page) and a brighter emerald on dark (`#6ee7b7`). The colour is a **distinct hue from `--accent`** (which colours markdown links), so code and links stay separable even side by side; the mono family reinforces the distinction.
- The same unboxed-text recipe is applied to `.settings-help code`, preserving the ONE-inline-code-recipe invariant.
- The previous `SHALL NOT take a --surface-2 fill` veto in the spec is dropped — it guarded a transparent-chip world that no longer exists. The orphaned `--code-bg` exploration token is removed.
- Fenced `pre` code blocks, blockquotes, body prose, links, and every other markdown treatment are **unchanged**.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `visual-identity`: the **Markdown Body Adopts the Type System** requirement is rewritten so inline code is unboxed `--code-fg`-coloured medium-weight mono text rather than an outlined chip. The `SHALL NOT take a --surface-2 fill` clause is removed; a per-scheme AA-contrast clause and a "distinct from `--accent`" clause are added. The "Inline code is an outlined chip" scenario is renamed to "Inline code is unboxed coloured text" with matching assertions. Fenced-block and body-text requirements/scenarios are untouched.

## Impact

- `src/App.css` — add `--code-fg` to `:root` (`#047857`) and the dark block (`#6ee7b7`); rewrite `.markdown-view code` to `font-mono` + `color: var(--code-fg)` + `font-weight: 500` + `font-size: 0.88em`, dropping `background`, `border`, `padding`, and `border-radius`; mirror the same on `.settings-help code`; remove the orphaned `--code-bg` token.
- `openspec/specs/visual-identity/spec.md` — synced from the delta on archive.
- **No Rust, no TypeScript, no IPC, no token-ledger changes beyond the single `--code-fg` addition.** Pure CSS.
- No change to fenced code blocks, blockquotes, links, body prose, or any non-markdown surface.
