# Design — Restyle: Engineered Minimal

## Context

SpecForge currently ships ~640 lines of hand-written CSS in `src/App.css` with five ad-hoc custom properties (`--row-hover`, `--row-selected`, `--text-muted`, `--divider`, `--divider-hover`), the system font stack, and `rgb(0, 122, 255)` (raw macOS system blue) as the only chromatic accent. Sizes (10/11/12/13/14px) and spacing values (2/4/6/8/10/12/24/32px) are inline magic numbers. Light/dark mode works via `color-scheme: light dark` plus translucent `rgba(127, 127, 127, …)` helpers — functional but flavorless.

The product is a tray-resident desktop utility built on Tauri 2 with a React + vanilla CSS frontend and a Rust core. Users do both quick triage (tray badge, tree scanning) and deep reading (detail pane), so the restyle has to serve both modes. The user has chosen an "engineered minimal" direction with Linear indigo (`#5e6ad2`) as the accent, cool-tinted neutrals, and Mac-first treatment with clean fallbacks on Windows/Linux.

## Goals / Non-Goals

**Goals:**
- Give SpecForge a recognizable visual identity without adding a UI framework or CSS-in-JS dependency.
- Introduce a real design token system (color, type, space, radii) that future UI work consumes for free.
- Adopt Inter and JetBrains Mono as the type system; vendor them locally; zero network at runtime.
- Make Linear indigo (`#5e6ad2`) the single accent. Status colors (green, red) are used sparingly.
- Replace tinted-fill selection with a 2px accent left bar + faint tint — instantly recognizable, very cheap.
- Outlined chip badges (no fill) for status indicators. Divergence collapses to a 4px dot in dense rows.
- Uniform row grammar: tree rows, settings workspace rows, future list surfaces share padding, type, and dividers.
- Enable macOS sidebar vibrancy and hidden-inset titlebar via `window-vibrancy`. Windows/Linux get solid sidebar + standard titlebar with the same color/type/selection rules.
- Replace placeholder glyphs (`▸ ▾ ● ✕`) with hand-rolled inline SVGs in one component.

**Non-Goals:**
- Markdown rendering beyond typography swap (no callouts, no anchor-link affordances, no custom code-block chrome). That's a follow-up change.
- Light/dark theme *toggle* in settings. We continue to follow the system `prefers-color-scheme`.
- Cmd-K command palette, search, or other features the aesthetic invites. Style only.
- Replacing vanilla CSS with Tailwind, CSS Modules, or any CSS-in-JS. Tokens stay as `:root` custom properties.
- Tray-icon SVG rework (existing template-image rasterizer stays; only the *menu* and *window chrome* change visually).
- Animations and motion design beyond the existing CSS transitions on hover/focus.

## Decisions

### D1. CSS custom properties as the token layer, not a build-time system

Tokens live as `:root` custom properties with `:root[data-theme="dark"]` (or `@media (prefers-color-scheme: dark)`) overrides. No PostCSS plugins, no Tailwind, no JS theming layer.

**Why over alternatives:**
- Tailwind: adds tooling, JIT config, and ~50KB of utility CSS. We don't need it; one stylesheet of ~500 lines is easier to own.
- CSS-in-JS (vanilla-extract, stitches): adds a runtime or build step for a feature CSS variables already provide for free.
- Style-dictionary / token build tool: overkill for ~50 tokens.

### D2. Token names follow a flat, semantic scheme

Color: `--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`, `--accent`, `--accent-hover`, `--accent-tint`, `--ok`, `--warn`.

Type: `--text-xs` (10px), `--text-sm` (11px), `--text-base` (12px), `--text-md` (13px), `--text-lg` (15px), `--text-xl` (20px), `--text-2xl` (28px). Mono uses the same scale.

Space: `--space-1` (4px), `--space-2` (8px), `--space-3` (12px), `--space-4` (16px), `--space-5` (24px), `--space-6` (32px), `--space-7` (48px).

Radii: `--radius-sm` (4px), `--radius` (6px), `--radius-md` (8px).

Borders: `--border-width` (1px). All borders are 1px hairlines, color via `--border` / `--border-strong`.

**Why over alternatives:**
- Tailwind's t-shirt sizes: chosen for familiarity; the team already thinks in `xs/sm/md/lg/xl`.
- Avoid namespaced names like `--color-text-primary` — semantic-flat (`--text`) reads cleaner in 640 lines of stylesheet.

### D3. Color values — cool neutrals + Linear indigo

```
accent           #5e6ad2        (Linear indigo)
accent-hover     #4f5bbf
accent-tint      rgba(94,106,210,0.10)   (selection bg, focus rings at 30%)

light                                    dark
  bg             #fbfcfd                 #0e1116
  surface        #ffffff                 #161a20
  surface-2      #f5f7fa                 #1c2128
  border         #e3e7ec                 #252b34
  border-strong  #c8cfd8                 #353c47
  text           #1c2128                 #e6e9ee
  text-muted     #5b6470                 #8b96a4
  text-faint     #8a939f                 #5b6470

ok               #10b981  (present, checked — dots only, never fill)
warn             #ef4444  (missing, diverged — outline chips, never fill)
```

`accent-tint` (10% alpha) is the only fill we use for selected rows. Hover background is `--surface-2`. Active row borders pick `--accent` directly.

**Why over alternatives:**
- Pure system blue (`rgb(0,122,255)`): generic and gives the app no identity.
- Vercel cobalt (`#2563eb`): clean but extremely common in this aesthetic right now.
- Vivid green/orange accents: more distinctive but collide with status semantics.
- True grays: read as "default Mac tool" — the slight blue tint is part of the engineered character.

### D4. Typography — Inter + JetBrains Mono, vendored locally as Variable woff2

UI font: Inter Variable (one woff2, ~95KB compressed). Mono font: JetBrains Mono Variable (~55KB compressed). Both ship in `src/assets/fonts/` and are declared via `@font-face` in `index.html` with `font-display: swap` and a metric-compatible system fallback (`-apple-system, BlinkMacSystemFont, Segoe UI` for Inter; `ui-monospace, SFMono-Regular, Menlo` for mono).

Mono is used as a *type system*, not just for code blocks:
- Change IDs (`worktree-git-worktree-support`)
- Branch names (`master`, `feat/x`)
- Timestamps (`2026-05-25 14:32`)
- Progress counters (`12 / 18`)
- File paths in settings
- Inline `code` and fenced code blocks (existing behavior)

These all line up vertically across rows, which is the dominant visual signal of "designed".

Line heights: `--leading-tight: 1.4` (chrome), `--leading-prose: 1.65` (markdown body), `--leading-code: 1.5` (code blocks).

**Why over alternatives:**
- System font stack only: no identity gain.
- Geist (Vercel): handsome but less distinctive in this layout; Inter is the better workhorse.
- IBM Plex: also strong but heavier in feel; we want crispness.
- Google Fonts CDN: introduces network dependency in an offline-capable desktop app. Vendoring is correct.

### D5. Selection model — 2px accent left bar + 10% accent-tinted background

Tree rows and any list-row-like surface use a 2px solid `--accent` left border (via `border-left` on the inner content, with the outer row keeping its margin) and `background: var(--accent-tint)`. Hover is `background: var(--surface-2)` only. Focus ring (for keyboard nav) is `outline: 2px solid var(--accent)` with `outline-offset: -2px`.

**Why over alternatives:**
- Tinted fill only (current): functional but reads as flat; not distinctive.
- Right-side or full border: visually noisier.
- Heavy left bar (4-6px): too aggressive for the density we have.

### D6. Badges — outlined chips, no fill; status compresses to a 4px dot

Status badges use `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase`, `letter-spacing: 0.05em`, mono font, `font-size: var(--text-xs)`, `padding: 0 var(--space-1)`. Color is `--text-muted` for neutral chips, `--warn` for problem states.

Where horizontal space is tight (e.g. divergence indicators inside a dense row), the badge collapses to a 4px circular dot in the same color. The dot is always paired with a `title` attribute carrying the full label.

**Why over alternatives:**
- Filled pill (current `row-badge-missing`, `row-divergence-diverged`): visually loud, fights with selection.
- Text-only label: too easy to miss when scanning.

### D7. Icons — five hand-rolled inline SVGs in `src/components/icons.tsx`

We need: `ChevronRight`, `ChevronDown`, `Settings`, `Close`, `Dot` (filled and outline variants for present/absent state). Each is an exported React component returning an inline SVG sized by `width`/`height` props (default 14px), `currentColor` fill, `stroke-width: 1.5`. ~80 lines total.

**Why over alternatives:**
- Lucide React: ~3KB tree-shaken for five icons. Reasonable, but introduces a dependency for a tiny surface.
- Continuing with text glyphs (`▸ ▾ ✕ ●`): they render inconsistently across platforms and look unintentional.

### D8. macOS window chrome — sidebar vibrancy + hidden-inset titlebar

Add the `window-vibrancy` crate (`tauri-plugin-window-vibrancy` or the standalone `window_vibrancy` crate) and call `apply_vibrancy(&window, NSVisualEffectMaterial::Sidebar, None, None)` from `setup()` on macOS only (gate with `cfg!(target_os = "macos")`).

Switch the main window's `title_bar_style` to `Overlay` so the traffic lights float over the sidebar at `top-left`. Reserve `--space-6` (32px) of top padding inside the sidebar root so the traffic lights have a safe drop zone.

The sidebar element gets `background: transparent` on Mac; everywhere else it gets `background: var(--surface)`.

**Why over alternatives:**
- No vibrancy: leaves identity on the table on the platform where it matters most.
- Full-window transparency: would require the detail pane to be transparent too, harming readability.
- Custom traffic-light positioning via JS: brittle; native overlay is the right primitive.

### D9. Theme switch follows system `prefers-color-scheme`

We continue using `color-scheme: light dark` declared on `:root`, with token overrides via `@media (prefers-color-scheme: dark)`. No in-app theme toggle.

**Why:** desktop apps that follow system theme feel native by default. A toggle is a feature, not a style decision, and belongs in a separate change.

### D10. Migration order — tokens first, then components, then native chrome

The CSS refactor lands first as one PR-worth of work: introduce the tokens at the top of `App.css`, then rewrite each rule to consume them, deleting the old custom properties at the end. Components don't change in this step; they pick up the new look via class names that already exist.

Step two: replace placeholder glyphs in components with the icon set. Step three: add `window-vibrancy` on macOS and update the sidebar background.

**Why this order:** the token refactor is purely a CSS change that can be visually QA'd before any structural component edits, and before any Rust changes. If we hated the result we could revert without touching components or the Tauri shell.

## Risks / Trade-offs

- **Font swap flash** → use `font-display: swap` + metric-compatible system fallback so first paint is correct in shape; the swap when Inter loads is imperceptible. Alternative `font-display: block` would risk a brief invisible-text frame.
- **Vibrancy quirks on macOS** → the `window-vibrancy` crate occasionally has issues with WKWebView background blending on older macOS versions. Mitigation: gate behind `cfg!(target_os = "macos")` and a try-best `Result` — if the call fails, the sidebar falls back to `var(--surface)` and the app still works.
- **Hidden inset titlebar changes window dragging** → Tauri's overlay titlebar still drags from the top; we need to ensure no interactive controls hug the top-left 80px (drag region) and the top-right corner (window-controls reserve, though on Mac the controls are top-left). Test before shipping.
- **Aesthetic dating** → "engineered minimal" with indigo + cool grays is a recognizable 2024–2026 look. In three years it may read as period-specific. Mitigation: tokens isolate the palette; swapping in a future accent or warmer neutrals is a 5-line change.
- **Two new font files (~150KB)** → not network-fetched (bundled in app), but inflates the installer by ~150KB. Acceptable for a Tauri desktop app; documented in proposal.
- **Cross-platform feel** → Mac users see vibrancy and inset titlebar; Windows/Linux users see solid sidebar + stock titlebar. They get the same color/type/selection system but lose the platform flourishes. This is the agreed trade-off (Mac-first).
- **Uniform row grammar may feel monotonous** → settings rows look like tree rows look like archived rows. Some users find that boring. Mitigation: the accent + mono + outlined chips give enough visual texture per row; if it does feel flat once shipped, the row template is the single thing to revisit.

## Migration Plan

This is a frontend / Tauri-shell change with no data model, IPC, or settings migration. Rollout is a single release.

1. **CSS pass** (largest diff): introduce tokens, rewrite all rules to consume them, remove the five legacy custom properties, ensure dark and light both look intentional.
2. **Icon pass**: add `src/components/icons.tsx`, replace glyphs across `WorkspaceTree.tsx`, `SettingsView.tsx`, `DetailPane.tsx`, `App.tsx`.
3. **Font pass**: vendor `woff2` files, declare `@font-face` in `index.html`, point `body`/`code` to the new families.
4. **Native chrome pass** (macOS only): add `window-vibrancy` dependency in `crates/specforge/Cargo.toml`, wire vibrancy in `lib.rs::run`, set `titleBarStyle: Overlay` in `tauri.conf.json`, add safe-area padding in the sidebar root.
5. **Smoke test**: `bun tauri dev` on macOS (light + dark + system-theme-flip); confirm tray badge and notifications unaffected; verify no regressions in `cargo test`.

Rollback: revert the PR. No persisted state changes; users won't notice anything but the visual.

## Open Questions

- **Variable font axis range**: do we need only `wght` (400/500/600), or should we expose Inter's optical sizing for the markdown headings? Default: `wght` only — keeps payload smaller and avoids axis-misuse bugs.
- **Code-block syntax highlighting**: existing `hljs` solarized-ish theme will look out of place against indigo + cool grays. Cheapest fix is retuning the highlight colors as part of this change; a fuller code-block treatment lives in a follow-up. Decision: retune `.hljs-*` colors to harmonize, but no chrome changes.
- **macOS-only vibrancy crate choice**: `tauri-plugin-window-vibrancy` vs the underlying `window-vibrancy` crate directly. The plugin is more idiomatic in Tauri 2 but adds another `tauri-plugin-*` dependency. Pending a quick check of plugin status in Tauri 2; default to direct crate if the plugin is stale.
