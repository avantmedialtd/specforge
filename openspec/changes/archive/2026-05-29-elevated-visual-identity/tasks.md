## 1. Design token layer

- [x] 1.1 Rewrite the `:root` block in `src/App.css` with the scheme-stable + light tokens: add `--surface-3`, the `--accent-active`/`--accent-strong`/`--accent-tint-strong`/`--accent-glow` family, the elevation tokens (`--border-hairline-top`, `--shadow-0..3`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`, `--shadow-focus`, `--sidebar-edge`), and the inverse-direction light accent values. Keep all locked type/space/radii/font tokens unchanged.
- [x] 1.2 Rewrite the `@media (prefers-color-scheme: dark)` block: four-step neutral ladder (`#0a0d12`/`#13171e`/`#1b212b`/`#242c38`), two-tier borders (`#2b323d` / `#6a7587`), brighter text (`#f1f4f8`/`#a3aebf`/`#7d889a`), vivid indigo accent family, brighter `--ok #34d399` / `--warn #f87171`, and the redefined deeper shadow alphas + dark accent rgba + `--border-hairline-top: rgba(255,255,255,0.05)`.
- [x] 1.3 Confirm no `rgba(127,127,127,…)` helper and no stray legacy tokens (`--row-hover`, `--row-selected`, `--divider`, `--divider-hover`) remain, and every UI rule references a token (literals only in token defs + the hljs palette + the spec-sanctioned `#fff` button label). Tokenized the new decorative literals as `--warn-tint` / `--accent-rule` / `--accent-line` / `--swatch-ring`.

## 2. App shell, sidebar, divider, scrollbars

- [x] 2.1 Add `box-shadow: var(--sidebar-edge)` to `.split-pane-left` (keep solid `--surface`, the `border-right`, and the macOS 32px top padding — no vibrancy).
- [x] 2.2 Update `.split-pane-divider:hover/:active` to fill `--border-strong` + a faint centered 1px accent line (`--accent-line`).
- [x] 2.3 Add the Elevated scrollbar rules (`::-webkit-scrollbar`/`-track`/`-thumb`/`-thumb:hover`): transparent track, `--surface-3` pill thumb with a 2px transparent inset, `--border-strong` on hover.

## 3. Tree rows — selection, hover, swatch

- [x] 3.1 Revise `.tree-row.selected` to add the `--accent-tint` wash, keep the 2px `--accent` bar, and add `box-shadow: var(--shadow-accent)`.
- [x] 3.2 Add `.tree-row.selected:hover` using `--accent-tint-strong`, placed AFTER both `.tree-row:hover` and `.tree-row.selected` so it wins.
- [x] 3.3 Add the inset dark containment ring to `.row-swatch` (`--swatch-ring`); leave swatch hues unchanged. Hover (`--surface-2`) and top-level hairline confirmed against the lifted base.

## 4. Global focus recipe

- [x] 4.1 Replace the flat `outline` on `.tree-row:focus-visible` and `.sidebar-footer-button:focus-visible` with `outline: none; box-shadow: var(--shadow-focus)`.
- [x] 4.2 Apply the same `:focus-visible` recipe to `.btn-primary`, `.btn-remove`, `.settings-close`, `.palette-swatch`, and the settings toggle checkbox. `.workspace-name-input` keeps its own filled focus treatment (intentionally excluded). No old flat outline rules linger.

## 5. Progress meter + chip/dot discipline

- [x] 5.1 Add `box-shadow: var(--glow-ok)` to `.task-progress-fill` (track geometry + decorative `--border` unchanged).
- [x] 5.2 Verified `.chip`, `.row-count`, `.row-changeid`, `.row-branch`, and `.status-dot--ok/--warn` stay outlined/flat with NO glow; dots remain flat `currentColor`.

## 6. Markdown view

- [x] 6.1 `.markdown-view code`: dropped the `--surface-2` fill, kept `background: transparent`, added the 1px `--border` (honors the locked inline-code scenario). Reconciled `.settings-help code` to the identical recipe (+ `color: var(--text)`).
- [x] 6.2 `.markdown-view pre`: added 1px `--border` + `var(--shadow-2)` over `--surface` (lifted well); `pre code` stays transparent/borderless.
- [x] 6.3 `.markdown-view blockquote`: `--surface-3` fill, 3px `--accent-rule` left rule, `--text-muted` body, right-side radius.
- [x] 6.4 Links: `--accent` with `--accent-hover` on `:hover`; added the on-selected-row link override (`--accent-hover`). Retuned the `.hljs-*` palette (keyword `#c98ce0`, string `#6cc77a`, number `#e0a85c`, built_in `#e07a5f`; title = `--accent`; comment = `--text-faint`).
- [x] 6.5 Inspected `MarkdownView.tsx` — react-markdown emits a bare `<table>` (no wrapper), so kept `border-collapse: collapse` and skipped the shadow-card (safer path); th/zebra already `--surface-2`.

## 7. Settings view

- [x] 7.1 `.workspaces-list`: lifted with `var(--shadow-2)` (kept `--surface` + 1px `--border` + radius + `overflow:hidden`).
- [x] 7.2 `.workspace-row.missing`: 3px `--warn` left bar + `--warn-tint` band. `.workspace-name-input` hover → `--border-strong`; focus → `--surface-2` + `--accent` border + `var(--shadow-2)`.
- [x] 7.3 `.btn-primary`: `--accent-strong` fill + white label + `var(--shadow-accent)` + inner top-light; hover → `--accent-hover` + `--shadow-accent-strong`; active → `--accent-active` + `box-shadow:none`; disabled → opacity 0.6 + no shadow.
- [x] 7.4 `.btn-remove`: `var(--shadow-1)` outlined seat; hover → `--warn` border/text + `--warn-tint` fill. `.palette-swatch.selected`: 2px `--text` ring + a non-glowing solid 2px `--accent` ring; `.palette-swatch--none` dashed already uses `--border-strong`.
- [x] 7.5 `.settings-toggle-row:hover` and `.sidebar-footer-button:hover/.active` take the `--accent-tint` wash + `--text`. `.settings-error` and `.detail-pane-error` are `--warn-tint` bands (no glow). `.detail-pane-status`/`.settings-empty`/`.empty-state-body` ride the lifted `--text-muted` (no change needed).

## 8. Component parity

- [x] 8.1 Confirmed `src/components/DetailPane.tsx` and `src/components/SettingsView.tsx` carry no literal colors and consume only the new classes/tokens.

## 9. Spec sync

- [x] 9.1 After implementation is verified, apply the `visual-identity` delta into `openspec/specs/visual-identity/spec.md` at archive time (handled by `openspec archive`).

## 10. Build + live verification

- [x] 10.1 `bun run build` (tsc `--noEmit` + vite) passes with no type or bundle errors.
- [x] 10.2 Verified live via the running dev instance (HMR) in DARK mode: tree hierarchy, the selected-row indigo wash + bar (clear "pop"), swatches with containment ring, vivid `--ok` progress meters, sidebar/detail-pane separation, brighter text, and the detail-pane markdown with transparent outlined inline-code chips. Settings view + fenced-code wells were not individually navigated (no click-automation tool available) — they reuse the same `--shadow-2` / `--shadow-accent` system proven on screen; build-verified.
- [ ] 10.3 LIGHT mode not live-captured (would require flipping the OS appearance, disrupting the whole desktop). Build-verified + contrast-computed in `design.md` (inverse accent direction, half-alpha shadows, darkened `--text-faint`). Offer to capture on request.
- [x] 10.4 Text legibility confirmed against the rendered DARK result (muted/faint labels, change-id, mtime all clearly readable); full WCAG floors computed in `design.md`'s contrast table.
