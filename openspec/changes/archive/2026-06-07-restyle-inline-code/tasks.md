# Tasks

## 1. Token

- [x] 1.1 Add `--code-fg` to `:root` (light) — a darker emerald `#047857` that clears AA 4.5:1 on `--bg`
- [x] 1.2 Add `--code-fg` override in the dark block — a brighter emerald `#6ee7b7`
- [x] 1.3 Remove the orphaned `--code-bg` exploration token from both schemes

## 2. Inline-code recipe

- [x] 2.1 Rewrite `.markdown-view code`: `--font-mono`, `color: var(--code-fg)`, `font-weight: 500`, `font-size: 0.88em` — drop `background`, `border`, `padding`, `border-radius`
- [x] 2.2 Mirror the same recipe on `.settings-help code` (preserve the ONE-inline-code-recipe invariant)
- [x] 2.3 Confirm fenced `pre` / `pre code`, blockquotes, links, and body prose are untouched

## 3. Verification

- [x] 3.1 Verify `--code-fg` clears AA 4.5:1 on the markdown background in both schemes (light ~5.3:1, dark high-contrast)
- [x] 3.2 Confirm inline code (mono, emerald) is visually distinct from links (sans, `--accent`) where they co-occur
- [ ] 3.3 Visual check in the running app, light + dark, on a code-dense doc
- [x] 3.4 `bun run build` (tsc + bundle) passes — no type or build regressions

## 4. Spec sync

- [ ] 4.1 On archive, sync the `visual-identity` delta into `openspec/specs/visual-identity/spec.md`
