## ADDED Requirements

### Requirement: Pinned Values Describe the Mycelium Theme

This capability specifies the **Mycelium** theme of the `theme-system` capability. Every concrete value and treatment it pins — hex and rgba colours, radii, the Inter typeface, the elevation and glow recipes, the row-selection model, the chip and status-dot vocabulary, the task-progress meter, the markdown type pass and the syntax-highlight palette — SHALL hold when the Mycelium theme is active, and SHALL NOT be read as constraining the Forge, Blueprint or Folio themes.

Mycelium keeps its indigo identity but not its original density. Its type sizes, line heights, document body size and the spacing of its rows and controls SHALL be those of the Forge theme, so that Mycelium is as spacious as the default and no text in it renders smaller than 13px; and its monospaced typeface SHALL be Meslo (see the *Typography System* requirement). Where another requirement of this capability states a type size, a line height, a row height or a padding in pixels, or names JetBrains Mono, that figure describes Mycelium as it shipped before themes existed and is superseded by this paragraph; the colours, depth, selection model, chip vocabulary and label casing those requirements specify are unaffected.

What every theme shares is the token *vocabulary* (see the *Design Token Layer* requirement) and the legibility floors (see the *Every Theme Meets the Same Legibility Floors* requirement in the `theme-system` capability). Where a requirement of this capability states a contrast floor, that floor applies to Mycelium through this capability and to every theme through `theme-system`.

Parts of this capability that are not a matter of identity apply under every theme: the inline SVG icon set, the macOS hidden inset titlebar layout and its traffic-light clearance, and the uniform row grammar's structure (which elements a row has, and in what order).

#### Scenario: Mycelium renders the pinned values

- **WHEN** the Mycelium theme is active in the dark scheme
- **THEN** `--accent` resolves to `#7c8cff` and `--bg` to `#0a0d12`
- **AND** the UI typeface is Inter Variable and the monospaced typeface is Meslo

#### Scenario: Mycelium is as spacious as Forge

- **WHEN** the same view is rendered in Mycelium and in Forge
- **THEN** every type-size and line-height token resolves to the same value in both
- **AND** rows and controls have the same padding and height in both
- **AND** no text in Mycelium is smaller than 13px

#### Scenario: Another theme is not held to Mycelium's values

- **WHEN** the Forge theme is active
- **THEN** its accent, neutrals, typefaces, type sizes and selection treatment are Forge's own
- **AND** no requirement of this capability is violated by their differing from the values pinned here

#### Scenario: Identity-free structure holds in every theme

- **WHEN** the desktop application runs on macOS in any theme with the sidebar hidden
- **THEN** the detail pane reserves the traffic-light clearance this capability specifies

## MODIFIED Requirements

### Requirement: Design Token Layer

The application SHALL expose a single vocabulary of design tokens as CSS custom properties. The *names* are shared by every theme of the `theme-system` capability; the *values* SHALL be supplied by the active theme, with that theme's dark-scheme values applied when the resolved scheme is dark (see the *The Scheme Is a Selectable Preference* requirement in the `theme-system` capability). All UI chrome (sidebar, tree rows, detail pane, settings, badges, buttons, dividers) SHALL consume these tokens rather than inline literal color, size, spacing, or radius values, and rules that are not part of a theme SHALL contain no identity value of their own.

The token set SHALL include, at minimum:

- Color: `--bg`, `--surface`, `--surface-2`, `--surface-3`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`, `--accent`, `--accent-hover`, `--accent-active`, `--accent-strong`, `--accent-tint`, `--accent-tint-strong`, `--accent-glow`, `--ok`, `--warn`.
- Elevation: `--border-hairline-top`, `--shadow-0`, `--shadow-1`, `--shadow-2`, `--shadow-3`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`, `--shadow-focus`, `--sidebar-edge`.
- Type sizes: `--text-xs`, `--text-sm`, `--text-base`, `--text-md`, `--text-lg`, `--text-xl`, `--text-2xl`.
- Type families: `--font-ui`, `--font-mono`.
- Line heights: `--leading-tight`, `--leading-prose`, `--leading-code`.
- Space: `--space-1` (4px), `--space-2` (8px), `--space-3` (12px), `--space-4` (16px), `--space-5` (24px), `--space-6` (32px), `--space-7` (48px).
- Radii: `--radius-sm`, `--radius`, `--radius-md`.

Every theme SHALL define every token of the vocabulary in both schemes, so that no rule resolves a token to nothing under any theme. A theme that does not use a treatment SHALL define its token to the neutral value (for example an elevation token of `none`) rather than leave it undefined. The space scale is the same in every theme.

In the Mycelium theme the type-size and line-height tokens SHALL resolve to the Forge theme's values: type sizes `--text-xs` 13px, `--text-sm` 14px, `--text-base` 15px, `--text-md` 15px, `--text-lg` 17px, `--text-xl` 22px, `--text-2xl` 30px; line heights `--leading-tight` 1.5, `--leading-prose` 1.6, `--leading-code` 1.6. Its radii remain its own: `--radius-sm` 4px, `--radius` 6px, `--radius-md` 8px.

In the Mycelium theme the neutral surfaces SHALL form a four-step ladder `--bg` → `--surface` → `--surface-2` → `--surface-3`, each a genuinely distinct value so that stacked surfaces read as separate planes. `--border` SHALL be a quiet DECORATIVE hairline that is never the sole signal of a control boundary; `--border-strong` SHALL be the LOAD-BEARING control edge. The elevation tokens SHALL carry plane separation (a drop shadow plus a 1px inner top-light via `--border-hairline-top`) so that depth does not depend on the hairline.

The pre-existing legacy custom properties `--row-hover`, `--row-selected`, `--text-muted` (in its previous untyped form), `--divider`, and `--divider-hover` SHALL be removed once their callers consume the new tokens.

#### Scenario: Tokens defined on :root

- **WHEN** the application stylesheet is loaded and any theme is active
- **THEN** the root element resolves the full color, elevation, type, space, radii, and border token set to that theme's values
- **AND** every UI rule in the stylesheet references at least one token rather than a literal value (color literals are permitted only inside a theme's token definitions and inside a theme's syntax-highlight palette)

#### Scenario: Type-size tokens render at the retuned px values

- **WHEN** the application stylesheet is loaded and the Mycelium theme is active
- **THEN** `--text-xs` resolves to 13px, `--text-sm` to 14px, `--text-base` to 15px, `--text-md` to 15px, `--text-lg` to 17px, `--text-xl` to 22px, and `--text-2xl` to 30px
- **AND** `--leading-tight` resolves to 1.5

#### Scenario: Dark mode token overrides

- **WHEN** the resolved scheme is dark
- **THEN** the active theme's dark values override at least `--bg`, `--surface`, `--surface-2`, `--surface-3`, `--border`, `--border-strong`, `--text`, `--text-muted`, and `--text-faint`
- **AND** in the Mycelium theme the drop-shadow alphas and accent rgba of the elevation tokens are redefined for the dark scheme
- **AND** the accent and status tokens otherwise remain consistent in intent across light and dark

#### Scenario: Every theme defines the whole vocabulary

- **WHEN** each theme is activated in turn, in light and in dark
- **THEN** every token of the vocabulary resolves to a value under each

### Requirement: Typography System

In the Mycelium theme the application SHALL use Inter Variable as its UI typeface (`--font-ui`) and Meslo as its monospaced typeface (`--font-mono`). Both fonts SHALL be vendored locally as `woff2` files under `src/assets/fonts/` and declared via `@font-face` with `font-display: swap` and a metric-compatible system fallback stack. The typefaces of the other themes are specified by the *The Theme Is a Selectable Preference* requirement in the `theme-system` capability and SHALL be vendored and declared the same way.

Mono SHALL be used as a *type system* across the chrome — not only for code blocks but also for change identifiers, branch names, file paths, and timestamps — so that these elements line up vertically across rows. Task progress is no longer a textual element in the tree row (it renders as a fill meter — see the *Task Progress Meter* requirement), so it is not part of this mono type system.

#### Scenario: Fonts loaded from local assets, not the network

- **WHEN** the application is started offline
- **THEN** the active theme's typefaces render correctly — in Mycelium, Inter and Meslo
- **AND** no network request is made for font files

#### Scenario: Mono applies to identifier-like elements in the tree

- **WHEN** a workspace tree row displays a change ID, branch name, or mtime
- **THEN** that element is rendered in `--font-mono`
- **AND** identifier glyphs of equal character count align vertically across rows

## REMOVED Requirements

### Requirement: Theme Follows System Preference

**Reason**: The application now offers a scheme override (System, Light, Dark) and a choice of themes. Following the operating system is the behaviour of the System choice, which remains the default, rather than the only behaviour.

**Migration**: See the *The Scheme Is a Selectable Preference* requirement in the `theme-system` capability for scheme resolution, including native scrollbars and form controls following the resolved scheme, and the *Settings Are Organised Into Groups* requirement in the `settings-view` capability for where the control lives. An installation that never touches the setting behaves as before: it follows the operating system.
