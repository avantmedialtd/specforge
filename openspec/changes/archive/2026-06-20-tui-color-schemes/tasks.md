## 1. Slot model and resolved theme

- [x] 1.1 Define a `Slot` enum (accent, on_accent, border_focused, border_dim, text_dim, selection, status_error, status_warn, status_ok) in `theme.rs`
- [x] 1.2 Define a `Scheme` type that supplies a `(rgb, ansi16)` pair per slot plus the data palettes (workspace tints, lane colours, person spread)
- [x] 1.3 Introduce `ResolvedTheme { depth, emoji, scheme }` with `slot(Slot) -> Color` resolving through the existing `rgb()` fallback ladder; keep `depth`/`emoji` env-frozen
- [x] 1.4 Build the **Default** scheme from today's exact constants (`PALETTE_RGB`, `LANE_RGB`, `PALETTE_ANSI16`, `PERSON_COLORS`, `ACCENT = Cyan`) so it is pixel-identical to current behaviour

## 2. Behaviour-preserving refactor of call sites

- [x] 2.1 Thread `&ResolvedTheme` (owned by `Model`) into the render path in `ui.rs` and `graph.rs`, replacing ad-hoc `theme()` lookups
- [x] 2.2 Replace the `ACCENT` constant (both definitions) with `slot(Slot::Accent)` at all ~68 call sites
- [x] 2.3 Replace scattered inline `Color::{DarkGray, Red, Black, Green, Yellow}` with the matching slots (`text_dim`, `status_*`, `on_accent`)
- [x] 2.4 Move `quota_color`, `rarity`, `lane`, `person`, `header_style` onto the scheme, preserving their thresholds and meanings
- [x] 2.5 Add/extend `render_tests.rs` snapshots asserting the Default scheme output is unchanged from before the refactor

## 3. Cheap presets

- [x] 3.1 Add the **Terminal-native** scheme: every slot resolves to a named ANSI `Color` (or `Color::Reset`), never `Color::Rgb`
- [x] 3.2 Add the **Monochrome** scheme: distinctions via weight/reverse-video, colours resolve to `Reset`/`Gray`
- [x] 3.3 Add the **High-contrast** scheme
- [x] 3.4 Snapshot-test each preset: native emits only named colours; monochrome emits no colour

## 4. Persistence

- [x] 4.1 Add a terminal-only preference store (e.g. `<config_dir>/specforge/tui.json` with `colorScheme`) resolved via the existing config-dir resolver
- [x] 4.2 Load the persisted scheme at startup; unknown/absent falls back to Default
- [x] 4.3 Persist the scheme on change; verify it does not touch `AppSettings` or any workspace

## 5. Settings Appearance control

- [x] 5.1 Add an Appearance section to the Settings screen listing schemes and marking the active one
- [x] 5.2 Add a keybinding to cycle/select the scheme with live preview (no restart), avoiding clashes with existing Settings keys
- [x] 5.3 Apply the selection to `Model` so the next redraw reflects it; persist immediately

## 6. Data-palette presets (staged last)

- [x] 6.1 Add the **Nord** scheme: eight separable workspace tints + eight lane colours + slot values
- [x] 6.2 Add the **Gruvbox** scheme: eight separable workspace tints + eight lane colours + slot values
- [x] 6.3 Sanity-check workspace-tint separability and status-colour legibility across all presets
