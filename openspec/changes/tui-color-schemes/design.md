## Context

`crates/specforge-tui/src/theme.rs` already resolves terminal capability once at
startup into a process-wide `OnceLock<Theme>` (`depth: ColorDepth`, `emoji:
bool`) and downsamples every truecolor RGB through `Theme::rgb()` to 256/16/mono.
What it lacks is a notion of a *user-chosen palette*. Today the palettes are
fixed module constants (`PALETTE_RGB`, `LANE_RGB`, `PALETTE_ANSI16`,
`PERSON_COLORS`) tuned to the desktop brand, and the chrome accent is a separate
hardcoded `const ACCENT: Color = Color::Cyan` referenced ~68× across `ui.rs` and
once in `graph.rs`, alongside scattered inline `Color::{DarkGray, Red, Black,
Green, Yellow}` calls. There is no per-frontend preference store for a theme.

The renderer already re-renders the whole frame from `Model` on every tick, so a
runtime palette swap needs no special invalidation — it only needs the active
palette to be reachable from the render path and mutable from the Settings
screen.

## Goals / Non-Goals

**Goals:**
- A `Scheme` value type that supplies every semantic colour slot and data
  palette, plus a curated set of presets (Default, High-contrast, Monochrome,
  Nord, Gruvbox, Terminal-native).
- Collapse the `ACCENT` constant and scattered inline colours into named slot
  lookups so a scheme actually controls the look.
- Runtime scheme switching from the Settings screen, applied live and persisted
  per terminal frontend.
- Preserve the existing capability downsampling and `NO_COLOR`/mono behaviour —
  scheme choice feeds the *input* palette; depth/`NO_COLOR` still have the final
  say.

**Non-Goals:**
- Automatic light/dark background detection (e.g. via `COLORFGBG`). All v1
  schemes assume a dark background.
- User-defined / custom schemes loaded from a file. Only the curated presets.
- Theming the desktop app or changing `openspec-core`.
- Re-theming the per-workspace tint *semantics* — a scheme may restyle the eight
  tint hues, but the workspace→hue assignment stays owned by the existing
  `PaletteColor` presentation logic.

## Decisions

### 1. Global `Theme` + atomic active-scheme index + pure `Scheme` resolution

**(implemented)** Keep the env-resolved `OnceLock<Theme>` and its `theme() ->
&'static Theme` accessor unchanged. Add the *active scheme* to that same `Theme`
as an `AtomicU8` index (interior-mutable through the shared `&'static`), flipped
by `theme::set_scheme(..)` from the Settings picker and the startup persistence
load. Colours resolve through a **pure `Scheme` API** — `Scheme::slot(Slot,
&Theme)` and the scheme-aware `Theme::{palette_fg,lane,person,rarity,quota}` — so
correctness is unit-testable without ever mutating the global.

This replaces the originally-planned `ResolvedTheme { depth, emoji, scheme }`
owned by `Model` and threaded into every render fn. The call-site survey settled
it: **every** colour site already holds a `th` (a `let th = theme();` or a `th:
&theme::Theme` param), so `th.accent()` / `th.slot(..)` drop in with near-zero
threading, whereas a Model-owned value would have to be plumbed through ~15
helpers and into `graph.rs`/`markdown.rs` (which take neither `model` nor `th`) —
pure churn for no behavioural gain. The global's one real downside, a hidden
mutable global hurting parallel snapshot tests, is removed by keeping resolution
*pure*: scheme-correctness tests call `Scheme::Native.slot(..)` / a depth-pinned
`Theme` directly and never touch the process-wide active pointer. `Theme::slot`
also returns `Color::Reset` whenever `depth == Mono`, so `NO_COLOR` overrides any
scheme. The persisted choice lives in a TUI-only `tui.json` (see Decision 4),
loaded at startup before the first frame.

### 2. Semantic slots, not raw colours, at call sites

Introduce a `Slot` vocabulary the renderer paints against:

```
accent · on_accent · border_focused · border_dim · text_dim
selection · status_error · status_warn · status_ok
```

`ResolvedTheme::slot(Slot) -> Color` resolves through the active scheme and the
existing `rgb()` fallback ladder. `ACCENT` → `slot(Accent)`; the inline
`Color::DarkGray` → `slot(TextDim)`; `Color::Red`/`Yellow`/`Green` status colours
→ `status_*`. The existing semantic helpers (`quota_color`, `rarity`, `lane`,
`person`, `header_style`) move onto the scheme so a scheme can restyle them, but
their *thresholds and meanings* are unchanged.

### 3. A scheme carries RGB + an ANSI-16 floor; native short-circuits

Each scheme slot is defined as a truecolor `(u8,u8,u8)` plus a named `Color`
floor, exactly mirroring today's `rgb(triple, ansi16)` pairs — so downsampling is
unchanged. Two presets are special:

- **Terminal-native**: every slot resolves to a *named* ANSI `Color` (or
  `Color::Reset` for default ink), never `Color::Rgb`, so the user's terminal
  theme supplies the actual pixels. This is the cheapest preset because the
  named floors already exist (`PALETTE_ANSI16`, `PERSON_COLORS`).
- **Monochrome**: every slot resolves to `Color::Reset`/`Gray` with weight and
  reverse-video carrying the distinctions — effectively the existing `Mono`
  depth path, available as an explicit choice even on colour-capable terminals.

### 4. Persist per-terminal, not in shared `AppSettings`

The scheme is a terminal-frontend display preference the desktop never reads.
Store it in a small TUI-owned file in the shared config directory (e.g.
`<config_dir>/specforge/tui.json` → `{ "colorScheme": "nord" }`), resolved
through the same config-dir resolver the frontend already uses, rather than
extending `openspec_app::AppSettings`. This keeps the desktop's settings schema
untouched and is consistent with the Read-Only Operation requirement (config in
the shared config dir, never inside a workspace). Read at startup, written on
change.

### 5. Settings-screen Appearance control

Add an Appearance row/section to the Settings screen (screen `6`). It lists the
schemes and marks the active one; a key cycles (or arrows select) and the change
applies on the next frame as live preview, persisting immediately — the same
interaction shape as the existing `c` workspace-colour cycle. Exact keybinding
chosen during implementation to avoid colliding with the Settings screen's
existing bindings.

## Risks / Trade-offs

- **Wide mechanical churn** (~68 `ACCENT` sites + scattered colours + threading
  `&ResolvedTheme`) → land it as one focused refactor that is behaviour-preserving
  for the Default scheme, guarded by snapshot tests, *before* adding non-default
  presets.
- **Nord/Gruvbox palettes are subjective and slow to tune** → stage them last;
  the slot refactor, Default, Terminal-native, Monochrome and High-contrast form
  a shippable first wave, with Nord/Gruvbox added behind it without reopening the
  architecture.
- **Data-hue remap can hurt workspace distinguishability** (a scheme with low
  hue spread blurs which workspace is which) → presets must keep eight visually
  separable tints; assert separability informally during tuning, and Default
  keeps today's proven hues.
- **Per-terminal config divergence from desktop** → intentional; documented in
  the proposal. The only shared state remains the registry/presentation stores.

## Migration Plan

1. Land the slot refactor + `ResolvedTheme` with only the Default scheme wired —
   pixel-identical to today (snapshot tests pin this).
2. Add Terminal-native, Monochrome, High-contrast (cheap; reuse named floors).
3. Add the Settings Appearance control + persistence; default remains Default.
4. Add Nord, then Gruvbox data palettes.

No rollback concern: absent/unknown persisted scheme falls back to Default.

## Open Questions

- Exact config filename/key and whether to share the resolver helper with the
  workspace-presentation store or add a sibling.
- Settings keybinding for cycling schemes (avoid clashing with existing
  Settings-screen keys).
- Whether High-contrast is a distinct preset or a modifier layered on Default
  (leaning distinct preset for v1 simplicity).
