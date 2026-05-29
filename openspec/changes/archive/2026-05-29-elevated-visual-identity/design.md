## Context

SpecForge's dark UI reads as muted/faded/flat. The cause is structural, not accidental: the current `visual-identity` spec codifies three near-identical near-blacks (`#0e1116`/`#161a20`/`#1c2128`), near-invisible borders (`#252b34`), washed-out secondary text (`--text-muted #8b96a4`, `--text-faint #5b6470`), a "selection is a 2px bar with **no** background change" rule, and an "outlined, never filled" vocabulary with a single sanctioned fill (the progress meter). The accent (`#5e6ad2`) barely appears in practice.

The owner picked the boldest of three offered directions ("Elevated"). A multi-agent design pass (four senior-designer directions → judge panel → synthesis → contrast/aesthetic/regression audits) produced **"Elevated Indigo"**, the design realized here. Every text and UI/graphical color pair was WCAG-verified (table below). This is a presentation-only change to `src/App.css` (plus token-parity checks in two components); no Rust/IPC/parser/behavior changes.

## Goals / Non-Goals

**Goals:**
- Kill the washed-out feel: real depth between surfaces, perceptible hierarchy, a confident accent — while staying a calm pro tool (not loud, not "generic-AI").
- Keep every WCAG-AA contrast obligation for informational text and load-bearing UI edges.
- Preserve the locked layout/type system (sizes, leading, space, radii, fonts) and platform contracts (solid no-vibrancy sidebar, hidden-inset titlebar, OS-driven theme).
- Keep the dogfooded spec honest: revise exactly the `visual-identity` requirements the new look changes.

**Non-Goals:**
- No layout/IA changes, no new components, no markdown features beyond depth (callouts, anchor links, custom code chrome stay out).
- No Rust, IPC, command, event, tray, or parser changes.
- Light scheme is retuned only enough to stay coherent; dark is the focus.
- No in-app theme toggle (still OS-driven).

## Decisions

### D1 — Four-step neutral ladder instead of a contrast-only retune
`--bg #0a0d12 → --surface #13171e → --surface-2 #1b212b → --surface-3 #242c38` (step ratios ~1.08/1.11/1.15). `--surface-3` is a **new** token for the top inset well (scrollbar thumb, blockquote/aside cards). *Alternative considered:* keep three surfaces and just brighten text — rejected because the flatness comes from surfaces not separating, which a text-only change cannot fix.

### D2 — Two-tier hairline; decorative `--border` is intentionally below 3:1
Split the single border into a quiet decorative `--border #2b323d` and a load-bearing `--border-strong #6a7587` (≥3:1 on **all four** planes: surface 3.86, surface-2 3.47, surface-3 3.02, bg 4.18). Every control edge that is the *sole* boundary signal (input hover, divider hover, dashed "none" swatch) routes to `--border-strong`; plane separation is otherwise carried by elevation shadows + the inner top-light, not the hairline. **This is the owner-flagged call:** `--border` stays ~1.3–1.5:1 by design — that is the cure for "near-invisible borders" (you stop relying on the hairline), not a contrast regression. *Alternative:* push `--border` to ~`#3a4250` to literally clear 3:1 — rejected as visually noisy; the documented two-tier exception is the better answer.

### D3 — Elevation via box-shadow tokens + a 1px inner top-light
`--shadow-1/2/3` (deeper drop-shadow alphas in dark) each include `inset 0 1px 0 var(--border-hairline-top)` so raised surfaces catch a faint top edge. Applied to settings cards, fenced code wells, blockquote asides. `--sidebar-edge` is an inset right-edge highlight so the solid sidebar reads as the front plane. *Alternative:* heavier borders for depth — rejected (reintroduces the noisy look D2 avoids).

### D4 — Fill + glow appear in exactly three places
The relaxation of "outlined never filled" is deliberately narrow. Filled/glowing: **(1)** the selected tree row (`--accent-tint` wash + 2px `--accent` bar + `--shadow-accent` glow), **(2)** the primary button (`--accent-strong` fill, white label, glow), **(3)** the in-progress meter (`--ok` fill + faint `--glow-ok` halo). Plus focused inputs and the focus ring. **Everything else stays ink/outline** — informational chips (`changeId`/branch/`DIVERGED`/count) and the 4px status dots carry no glow. This keeps "accent = active" legible and the row calm. The audit explicitly *removed* proposed status-dot and selected-swatch glows to hold this line (the selected swatch uses a non-glowing solid 2px `--accent` ring; `.row-swatch` gets an inset *containment* ring, not a glow).

### D5 — Vivid two-saturation indigo with directional hover
`--accent #7c8cff` is the ink/line color (links, selection bar, focus, hljs titles, checkbox). `--accent-strong #4f5fe0` is the only fill-under-white-text surface (primary button; white = 5.19:1). Hover **brightens** on dark (`#93a1ff`) and **darkens** on light (`#3f4bc4`) — "lift" is direction-dependent on the scheme. Stays in the indigo/violet family (~H231); raw macOS system blue remains banned.

### D6 — Global focus recipe replaces the flat outline
`--shadow-focus: 0 0 0 2px var(--bg), 0 0 0 4px var(--accent), 0 0 0 7px var(--accent-glow)` (a bg-gap → accent ring → glow halo) replaces every `outline: 2px solid --accent; outline-offset:-2px`. The existing `.tree-row:focus-visible` and `.sidebar-footer-button:focus-visible` rules are **replaced** (set `outline:none` + the box-shadow), not duplicated. The accent ring on `--bg` is 6.53:1.

### D7 — Inline code stays transparent; fenced `pre` becomes a well
The "inline code is an outlined chip with a transparent background" scenario is locked. Inline `code` therefore **drops** its current `--surface-2` fill and **adds** a 1px `--border` (one recipe shared with `.settings-help code`). Only fenced `pre` blocks get the `--surface` well + `--shadow-2` + top-light. Blockquotes become `--surface-3` aside cards with a 3px accent-at-0.7 left rule.

### D8 — macOS sidebar edge is a shadow, not a material
The front-plane look uses `--sidebar-edge` (an inset box-shadow on the same solid `--surface`). No `NSVisualEffectView`/vibrancy is introduced; the solid-background lock and 32px traffic-light safe area are untouched.

### Token reference (implementation source of truth)

Dark-scheme neutrals + accent + status:
```
--bg #0a0d12  --surface #13171e  --surface-2 #1b212b  --surface-3 #242c38
--border #2b323d (decorative)  --border-strong #6a7587 (load-bearing)
--text #f1f4f8  --text-muted #a3aebf  --text-faint #7d889a
--accent #7c8cff  --accent-hover #93a1ff  --accent-active #5d6ef0  --accent-strong #4f5fe0
--accent-tint rgba(124,140,255,0.14)  --accent-tint-strong rgba(124,140,255,0.22)  --accent-glow rgba(124,140,255,0.35)
--ok #34d399  --warn #f87171
```
Scheme-stable / elevation (on `:root`, dark redefines shadow alphas + accent rgba):
```
--border-hairline-top  rgba(255,255,255,0.7) light / 0.05 dark
--shadow-0 none
--shadow-1/2/3  drop-shadow + inset top-light (deeper alphas in dark)
--shadow-accent / --shadow-accent-strong  1px accent edge + wide low-alpha glow
--glow-ok  0 0 8px -1px rgba(52,211,153,0.45) dark
--shadow-focus  0 0 0 2px var(--bg), 0 0 0 4px var(--accent), 0 0 0 7px var(--accent-glow)
--sidebar-edge  inset -1px 0 0 0 rgba(255,255,255,0.03) dark / rgba(0,0,0,0.04) light
```
Light scheme adds `--surface-3 #edf0f5`, the inverse-direction accent family (`--accent #4f5bd9` → hover `#3f4bc4`), darkened `--text-faint #6f7a86` (~4.7:1), and ~half-alpha drop-shadows. Locked tokens (type sizes, leading, space, radii, fonts, `--dim-opacity 0.45`) are unchanged.

### Contrast verification (WCAG, dark scheme)

All informational text and load-bearing UI/graphical pairs pass AA:
```
text on bg/surface/surface-2/surface-3   17.64 / 16.28 / 14.65 / 12.76
muted on surface/surface-2/surface-3/bg   8.01 / 7.21 / 6.27 / 8.69
faint on surface/surface-2                 5.01 / 4.51   (changeId/mtime/hljs-comment)
accent(link) on surface/bg                 6.03 / 6.53
accent on selection wash #22273e           4.94   (bar/links→use accent-hover 6.15)
white on accent-strong (button)            5.19
ok on surface/surface-2                    9.34 / 8.41
warn on surface / warn-tint band           6.49 / 5.70
border-strong on surface/surface-2/surface-3/bg   3.86 / 3.47 / 3.02 / 4.18
focus ring (accent on bg)                  6.53
hljs keyword/string/number/title/built_in/comment   6.40 / 7.77 / 7.64 / 5.43 / 5.48 / 4.51
```
Two pairs intentionally land in the 3:1–4.5 decorative/transient band and are recorded so the audit has no silent gap: `changeId` (`--text-faint`) on the selection wash = 4.07 (never the sole carrier of critical info; body text there is 13.33), and accent-as-ink on the *hover-over-selected* wash = 4.26 (transient state). The decorative `--border` is ~1.3–1.5:1 by design (D2).

## Risks / Trade-offs

- **Box-shadow focus/selection halos are clipped by `overflow:hidden`/`auto` ancestors** (`.workspaces-list`, `.sidebar-tree`) → Accepted: the 2px accent ring/bar stays visible and AA-compliant (4.94–6.53:1). Do **not** remove `overflow:hidden` to chase the halo.
- **Table-as-card needs `border-collapse:separate`**, which changes cell-border rendering → Defer: only wrap tables as `--shadow-2` cards if the renderer wraps tables in a container; otherwise keep `collapse` and skip the outer radius. Confirm `MarkdownView.tsx` table markup before applying (Open Question).
- **Light-scheme coherence** — light gets the new tokens + inverse-direction accent + half-alpha shadows; verify it still reads clean (dark is the priority).
- **Rule order matters** — `.tree-row.selected:hover` (uses `--accent-tint-strong`) must come **after** both `.tree-row:hover` and `.tree-row.selected`; `btn-primary:active` sets `box-shadow:none` after the rest/hover shadows. A mis-order silently drops the intended state.
- **Reduced motion** — the existing meter-fill transition guard stays; no new always-on animation is introduced (glows are static box-shadows).

## Migration Plan

1. Apply the token blocks + per-surface rule changes to `src/App.css` (see `design.md` reference + the change's source artifacts).
2. Confirm `DetailPane.tsx` / `SettingsView.tsx` error/status/empty classes carry no literal colors (token-only).
3. `bun run build` (tsc + bundle) green; verify live in `bun tauri dev` across the tree, detail pane (markdown/code/tables), settings, hover/selected/focus states, light + dark.
4. Rollback is a single-file CSS revert; archiving the change applies the delta into `openspec/specs/visual-identity/spec.md`.

## Open Questions

- **Table card:** wrap markdown tables as a `--shadow-2` card, or leave `border-collapse:collapse` and skip it? Decide after inspecting `MarkdownView.tsx` table output. Defaulting to "leave collapse" unless the renderer already wraps tables.
