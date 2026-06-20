# Terminal UI Colour Schemes

## Why

The terminal frontend hardcodes a single dark-tuned look. One accent colour
(`Color::Cyan`, referenced ~68× across the renderer) carries every focused
border, title bar, key hint and cursor, and fixed RGB palettes for workspace
tints and commit-graph lanes are imposed regardless of the terminal the user
has carefully themed. Someone running Nord, Gruvbox or Solarized gets a
SpecForge that clashes with everything around it, and there is no way to choose
a higher-contrast or monochrome look. The colour *plumbing* — capability
detection and RGB→256→16→mono downsampling in `theme.rs` — is already solid;
what is missing is user choice over the palette that plumbing feeds.

## What Changes

- Introduce a **named colour scheme** the user selects in the Settings screen,
  persisted across sessions and applied live without a restart.
- Ship a curated set: **Default** (today's brand look), **High contrast**,
  **Monochrome**, **Nord**, **Gruvbox**, and **Terminal-native** — the last
  deferring to the terminal's own ANSI palette instead of imposing RGB, so
  SpecForge blends into an already-themed terminal.
- Define the renderer's colours as a fixed set of **semantic slots** (accent,
  on-accent, focused border, dim border, dim/secondary text, error, warning,
  success, selection) plus **data palettes** (workspace tints, commit-graph
  lanes, person hues). A scheme supplies a value for every slot; capability
  downsampling and `NO_COLOR` still apply on top and always win.
- All schemes target a **dark background** in this version — no automatic
  light/dark detection. (A light scheme can follow later.)
- Stage the work so the **semantic slots, Terminal-native, Monochrome and
  High-contrast** land first; the hand-tuned **Nord/Gruvbox** data palettes
  follow without blocking that first wave.

## Capabilities

### New Capabilities
<!-- none — this extends the existing terminal-ui capability -->

### Modified Capabilities
- `terminal-ui`: adds a **Colour Scheme Selection** requirement; the **Settings
  Screen** requirement gains an Appearance control for choosing the scheme; the
  **Graceful Degradation** requirement is clarified so scheme choice composes
  with — and never overrides — capability downsampling and `NO_COLOR`.

## Impact

- `crates/specforge-tui/src/theme.rs` — gains a `Scheme` / resolved-theme
  abstraction with semantic slots; the `theme()` singleton is split so colour
  *depth* and *emoji* stay env-frozen while the *active scheme* becomes
  runtime-mutable.
- `crates/specforge-tui/src/ui.rs` and `graph.rs` — the `ACCENT` constant and
  the scattered inline `Color::{DarkGray, Red, Black, Green, Yellow}` calls are
  replaced by slot lookups on the resolved theme.
- `crates/specforge-tui/src/app.rs` — the Settings screen gains an Appearance
  section and a key to cycle schemes; the choice is persisted and restored.
- New **terminal-only** preference storage for the selected scheme (not added to
  the desktop's shared `AppSettings`; the desktop has no use for a terminal
  scheme). Exact location is settled in design.
- No change to `openspec-core` or the desktop app.
