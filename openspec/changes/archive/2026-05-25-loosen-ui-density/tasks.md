## 1. Update design tokens in `src/App.css`

- [x] 1.1 Bump the type-size tokens on `:root` to the new px values: `--text-xs` 10→12, `--text-sm` 11→13, `--text-base` 12→14, `--text-md` 13→15, `--text-lg` 15→16 (originally 17; pulled back during apply so markdown body sits closer to 15px code), `--text-xl` 20→22, `--text-2xl` 28→30.
- [x] 1.2 Update `--leading-tight` from 1.4 to 1.5; leave `--leading-prose` (1.65) and `--leading-code` (1.5) untouched.
- [x] 1.3 Sanity-check that nothing in `App.css` references a literal `13px`, `15px`, `1.4`, etc. that bypasses the tokens — if it does, route it through the token instead of leaving a hard-coded number behind.

## 2. Retune list-row vertical padding

- [x] 2.1 Change `.tree-row` padding from `2px var(--space-2) 2px var(--space-1)` to `5px var(--space-2) 5px var(--space-1)`.
- [x] 2.2 Change `.workspace-row` padding from `var(--space-3)` to `var(--space-4)`.
- [x] 2.3 Change `.settings-toggle-row` vertical padding from `4px 0` to `6px 0`.
- [x] 2.4 Widen `.markdown-view` `max-width` from `760px` to `880px` so prose at the new 16px body produces ~100 chars per line and fenced code blocks at 15px mono accommodate ~97 chars before horizontal scroll — balancing prose readability against code-block headroom in the detail pane.

## 3. Mirror the change in the visual-identity spec

The main `openspec/specs/visual-identity/spec.md` is not edited during apply. The delta in `specs/visual-identity/spec.md` (in this change directory) captures the intended modifications and will be synced into the main spec by `/opsx:archive` (or `/opsx:sync`) when the change is closed.

- [x] 3.1 Confirm the delta spec at `openspec/changes/loosen-ui-density/specs/visual-identity/spec.md` covers: MODIFIED Design Token Layer (new px values + new scenario for retuned token values), MODIFIED Markdown Body (px annotation 15 → 17), ADDED List-Row Vertical Rhythm requirement. Sync at archive will apply these to the main spec.

## 4. Verify the rendered result

- [x] 4.1 Run `bun run build` (this also runs `tsc --noEmit`) to confirm no incidental TypeScript regressions.
- [x] 4.2 Run `bun tauri dev` on the developer's 4K @ 100% workstation and visually confirm: sidebar tree rows read comfortably, settings list breathes, markdown reader is unchanged in shape, the chip/count meta is still legible at the new mono size. (Confirmed visually during apply; the markdown reader was then re-tuned mid-apply — body 17→16, max-width 760→880 — to balance prose against fenced-code blocks.)
- [x] 4.3 Eyeball the sidebar at the default split width: a typical workspace's full set of rows (workspace → change → 4 artifacts) is still visible without scrolling.
- [x] 4.4 If the mono `--text-xs` (12px) chips start to dominate visually next to the new UI text (15px), note it as a follow-up — do not adjust in this change. (No adjustment made; no follow-up reported by the user.)
