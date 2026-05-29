# Elevated Visual Identity

## Why

The app reads as muted, faded, and flat — surfaces don't separate, borders are near-invisible, secondary text is washed-out grey, and the single indigo accent barely appears. That low-contrast restraint is currently codified in the `visual-identity` spec (selection is a bar with *no* background, "outlined never filled" everywhere, three near-identical near-blacks). The owner has chosen the boldest direction: keep the calm pro-tool character but give it real depth, a confident accent, and crisp hierarchy so the UI feels modern and "pops."

## What Changes

- **Deeper, genuinely-separated neutral ladder** — replace the three near-identical near-blacks with a four-step dark ladder (`--bg #0a0d12` → `--surface #13171e` → `--surface-2 #1b212b` → `--surface-3 #242c38`, the last a **new** token) so stacked surfaces read as distinct planes.
- **Two-tier hairline system** — split the single border into a quiet decorative `--border` (`#2b323d`) and a load-bearing `--border-strong` (`#6a7587`, ≥3:1 on all four planes). Every control edge routes to `--border-strong`; plane separation is carried by elevation, not the hairline. **Decision recorded:** the decorative `--border` is intentionally below 3:1 — the two-tier strategy is the deliberate cure for "near-invisible borders," not a contrast regression.
- **Brighter, AA-clean text** — `--text #f1f4f8`, `--text-muted #a3aebf` (8.0:1 on surface), `--text-faint #7d889a` (≥4.5:1 where it carries changeId/mtime), ending the washed-out feel.
- **Vivid indigo accent system** — `--accent #7c8cff` (ink/lines), `--accent-hover #93a1ff`, `--accent-active #5d6ef0`, `--accent-strong #4f5fe0` (button fill, white label 5.19:1), plus `--accent-tint`, `--accent-tint-strong`, and `--accent-glow`. Stays in the indigo family; the raw macOS system blue is still banned.
- **Elevation system (NEW)** — box-shadow tokens `--shadow-0..3`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`, `--shadow-focus`, `--sidebar-edge`, and `--border-hairline-top` (a 1px inner top-light), giving the detail-pane code wells, settings cards, and the sidebar real depth.
- **Disciplined fill + glow** — fill/glow appear in **exactly three places**: the selected tree row (accent-tint wash + 2px bar + soft glow), the primary button (filled + glow), and the in-progress meter (`--ok` fill + faint halo); plus focused inputs and the focus ring. **BREAKING (selection):** the selected row now carries an `--accent-tint` background wash + glow, lifting the prior "selection is the 2px bar with no background change" prohibition.
- **Everything else stays ink/outline** — informational chips (`changeId`, branch, `DIVERGED`, count) and the 4px status dots remain outlined/transparent and carry **no** glow, so a busy row stays calm and the accent always means "active."
- **Brighter status colors** — `--ok #34d399`, `--warn #f87171`; missing-workspace and error surfaces become contained warn-tinted bands (no glow).
- **Markdown depth** — fenced `pre` blocks become lifted wells (`--surface` + `--shadow-2` + top-light); inline `code` **stays transparent** (honoring the locked outlined-chip scenario); blockquotes become `--surface-3` aside cards.
- **macOS sidebar front-plane edge** — a 1px inner right-edge box-shadow (`--sidebar-edge`) so the still-solid sidebar reads as the front plane. The solid-`--surface` lock and no-vibrancy rule are unchanged (this is a shadow on the same solid surface, not a material).
- **Unchanged / locked** — all type sizes, line-heights, the space scale, radii, Inter + JetBrains Mono families, the cool/blue neutral tint, no macOS system blue, the hidden-inset titlebar + 32px safe area, OS `prefers-color-scheme` with no in-app toggle, and `--dim-opacity 0.45` for missing-artifact rows.

## Capabilities

### New Capabilities
<!-- None — this change retunes an existing capability. -->

### Modified Capabilities
- `visual-identity`: Revises the **Accent Color** (new indigo system + filled/glowing-in-three-places rule), **Tree Row Selection Model** (adds the accent-tint wash + glow), **Design Token Layer** (adds `--surface-3`, the accent-active/strong/tint-strong/glow tokens, the elevation tokens, and the two-tier border roles), **Cool Neutral Palette** (new dark hex values + AA floors + below-3:1 decorative-border exception), **Outlined Chip Badges** (clarifies glow is reserved for the three active surfaces; chips/dots stay outlined), **Task Progress Meter** (adds a faint `--glow-ok` allowance), **macOS Hidden Inset Titlebar Layout** (adds the inner-edge highlight on the still-solid sidebar), and **Markdown Body Adopts the Type System** (lifted `pre` wells; inline code stays transparent) requirements.

## Impact

- **`src/App.css`** — the `:root` + dark-scheme token blocks and per-surface rules (sidebar, tree rows, selection, focus, markdown, settings, buttons, meter, scrollbars).
- **`openspec/specs/visual-identity/spec.md`** — applied from the delta after archive.
- **`src/components/DetailPane.tsx`** / **`src/components/SettingsView.tsx`** — minor parity touch-ups only (error/status/empty classes already consume tokens; confirm no literal colors remain).
- **No Rust / IPC / behavior changes** — this is a presentation-layer change. No type, command, event, or parser changes.
- **Accessibility** — every informational text and UI/graphical pair is WCAG-AA verified (see `design.md` contrast table).
