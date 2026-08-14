# visual-identity Specification

## Purpose

Defines SpecForge's desktop visual identity: the design-token layer (color, type, space, radii, borders), the typography system, the single accent color, the row-selection model, the outlined chip / status-dot vocabulary, the inline SVG icon set, the macOS window-chrome treatment (sidebar vibrancy + hidden inset titlebar), and the markdown-body type pass. Cross-cuts every UI surface — the workspace tree, detail pane, settings view, and any future list surface all consume the same tokens and follow the same row grammar. Source of truth for implementation choices like Linear indigo `#5e6ad2` as the accent, Inter + JetBrains Mono as the type families, and the 2px-accent-bar selection style.
## Requirements
### Requirement: Design Token Layer

The application SHALL expose a single source of design tokens as CSS custom properties declared on `:root`, with dark-scheme overrides scoped to `@media (prefers-color-scheme: dark)`. All UI chrome (sidebar, tree rows, detail pane, settings, badges, buttons, dividers) SHALL consume these tokens rather than inline literal color, size, spacing, or radius values.

The token set SHALL include, at minimum:

- Color: `--bg`, `--surface`, `--surface-2`, `--surface-3`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`, `--accent`, `--accent-hover`, `--accent-active`, `--accent-strong`, `--accent-tint`, `--accent-tint-strong`, `--accent-glow`, `--ok`, `--warn`.
- Elevation: `--border-hairline-top`, `--shadow-0`, `--shadow-1`, `--shadow-2`, `--shadow-3`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`, `--shadow-focus`, `--sidebar-edge`.
- Type sizes: `--text-xs` (12px), `--text-sm` (13px), `--text-base` (14px), `--text-md` (15px), `--text-lg` (16px), `--text-xl` (22px), `--text-2xl` (30px).
- Type families: `--font-ui`, `--font-mono`.
- Line heights: `--leading-tight` (1.5), `--leading-prose` (1.65), `--leading-code` (1.5).
- Space: `--space-1` (4px), `--space-2` (8px), `--space-3` (12px), `--space-4` (16px), `--space-5` (24px), `--space-6` (32px), `--space-7` (48px).
- Radii: `--radius-sm` (4px), `--radius` (6px), `--radius-md` (8px).

The neutral surfaces SHALL form a four-step ladder `--bg` → `--surface` → `--surface-2` → `--surface-3`, each a genuinely distinct value so that stacked surfaces read as separate planes. `--border` SHALL be a quiet DECORATIVE hairline that is never the sole signal of a control boundary; `--border-strong` SHALL be the LOAD-BEARING control edge. The elevation tokens SHALL carry plane separation (a drop shadow plus a 1px inner top-light via `--border-hairline-top`) so that depth does not depend on the hairline.

The pre-existing legacy custom properties `--row-hover`, `--row-selected`, `--text-muted` (in its previous untyped form), `--divider`, and `--divider-hover` SHALL be removed once their callers consume the new tokens.

#### Scenario: Tokens defined on :root

- **WHEN** the application stylesheet is loaded
- **THEN** `:root` declares the full color, elevation, type, space, radii, and border token set
- **AND** every UI rule in the stylesheet references at least one token rather than a literal value (color literals are permitted only inside token definitions themselves and inside the syntax-highlight palette)

#### Scenario: Type-size tokens render at the retuned px values

- **WHEN** the application stylesheet is loaded
- **THEN** `--text-xs` resolves to 12px, `--text-sm` to 13px, `--text-base` to 14px, `--text-md` to 15px, `--text-lg` to 16px, `--text-xl` to 22px, and `--text-2xl` to 30px
- **AND** `--leading-tight` resolves to 1.5

#### Scenario: Dark mode token overrides

- **WHEN** the operating system reports `prefers-color-scheme: dark`
- **THEN** the dark-scheme media query overrides at least `--bg`, `--surface`, `--surface-2`, `--surface-3`, `--border`, `--border-strong`, `--text`, `--text-muted`, and `--text-faint`
- **AND** it redefines the drop-shadow alphas and accent rgba of the elevation tokens for the dark scheme
- **AND** the accent and status tokens otherwise remain consistent in intent across light and dark

### Requirement: Accent Color

The application SHALL use a vivid indigo accent system in the indigo/violet family. The raw macOS system blue (`rgb(0, 122, 255)`) MUST NOT appear in user-visible chrome.

In the dark scheme the accent tokens SHALL be: `--accent` `#7c8cff` (the ink/line color used for links, the selection bar, focus rings, syntax-highlight titles, and the settings-toggle checkbox `accent-color` — markdown task checkboxes are status glyphs in the ok family per the *Markdown Task-Checkbox Treatment* requirement, not accent-coloured controls); `--accent-hover` `#93a1ff` (hover BRIGHTENS on dark); `--accent-active` `#5d6ef0` (pressed); `--accent-strong` `#4f5fe0` (the fill-under-white-text surface, used ONLY as the primary-button background, where the white label SHALL reach at least 4.5:1 — `#4f5fe0` yields 5.19:1); `--accent-tint` `rgba(124, 140, 255, 0.14)`; `--accent-tint-strong` `rgba(124, 140, 255, 0.22)`; `--accent-glow` `rgba(124, 140, 255, 0.35)`.

In the light scheme the accent SHALL hold its indigo identity with the INVERSE hover direction (on a light background, lift = darken): `--accent` `#4f5bd9`, `--accent-hover` `#3f4bc4`, `--accent-active` `#3a46b8`.

The accent SHALL appear FILLED or GLOWING in exactly four places — the selected tree row, the primary button, the in-progress task-progress meter, and the favorite star on a favorited change row (a solid `--accent` star glyph, glow-free; see the *Change-Row Favorite Toggle* requirement in the `spec-browser` capability) — plus focused inputs and the focus ring. Everywhere else (links, informational chips, status dots) the accent or status color SHALL be ink/outline only and SHALL carry no fill or glow. The sanctioned *done* fills — the completion mark in the row grammar and the checked markdown task checkbox in the document surface (see the *Outlined Chip Badges* and *Markdown Task-Checkbox Treatment* requirements) — are the only status-colour fills beyond the in-progress meter's sanctioned `--ok` fill (see the *Task Progress Meter* requirement), and both carry no glow.

#### Scenario: Accent stays in the indigo family

- **WHEN** the application stylesheet is inspected
- **THEN** the `--accent` family resolves to an indigo/violet hue (approximately hue 231)
- **AND** the raw macOS system blue `rgb(0, 122, 255)` does not appear in any user-visible chrome

#### Scenario: Accent is filled or glowing in exactly four places

- **WHEN** the UI is rendered
- **THEN** an accent fill or glow appears only on the selected tree row, the primary button, the in-progress meter, the favorited row's solid star, focused inputs, and the focus ring
- **AND** the favorite star's fill carries no glow
- **AND** links, informational chips, and status dots render their color as ink/outline with no fill or glow

#### Scenario: Primary button uses the accent

- **WHEN** a primary button is rendered (for example "Add workspace" in settings)
- **THEN** its background is `--accent-strong` with a white label at at least 4.5:1
- **AND** its hover background is `--accent-hover` and its pressed background is `--accent-active`

#### Scenario: Links in rendered markdown use the accent

- **WHEN** the detail pane renders an `<a>` element from markdown
- **THEN** the link color is `--accent`
- **AND** a link rendered on a selected row uses `--accent-hover` so it clears AA on the selection wash

### Requirement: Cool Neutral Palette

Neutral tokens SHALL carry a slight blue tint rather than pure gray, with no warm drift and no system blue. The dark-scheme neutrals SHALL be: `--bg` `#0a0d12`, `--surface` `#13171e`, `--surface-2` `#1b212b`, `--surface-3` `#242c38`, `--border` `#2b323d`, `--border-strong` `#6a7587`, `--text` `#f1f4f8`, `--text-muted` `#a3aebf`, `--text-faint` `#7d889a`. The previous `rgba(127, 127, 127, …)` helper pattern MUST NOT appear in the stylesheet.

The neutrals SHALL satisfy these contrast floors:

- `--text-muted` SHALL clear at least 4.5:1 on `--surface` and `--surface-2` (it carries information; 8.01 / 7.21).
- `--text-faint` SHALL clear at least 4.5:1 on `--surface` and `--surface-2` (it carries change-id and mtime, and `--surface-2` is the row-hover fill; 5.01 / 4.51).
- `--border-strong` SHALL clear at least 3:1 on all four neutral planes — `--surface` (3.86), `--surface-2` (3.47), `--surface-3` (3.02), and `--bg` (4.18).

The decorative `--border` is DELIBERATELY below 3:1 against the neutral planes. It SHALL NOT be the sole signal of any control boundary; every load-bearing edge SHALL route to `--border-strong`, and plane separation SHALL be carried by the elevation tokens. This two-tier strategy is the intended treatment for surface boundaries and is not a contrast defect.

In the light scheme `--text-faint` darkens to `#687380` (4.82:1 on `--surface`, 4.50:1 on `--surface-2`, 4.70:1 on `--bg` — the previous `#6f7a86` measured only 4.37:1 on `--surface`, under the faint floor above despite its recorded ~4.7:1) and `--surface-3` is `#edf0f5`; the other light neutrals are unchanged.

#### Scenario: No translucent gray helpers remain

- **WHEN** the final stylesheet is inspected
- **THEN** no rule uses `rgba(127, 127, 127, …)` as a fill, border, or text color
- **AND** all neutral surfaces resolve to one of the declared neutral tokens

#### Scenario: Informational neutral text clears AA

- **WHEN** `--text-muted` or `--text-faint` renders informational content on `--surface` or `--surface-2`
- **THEN** its contrast ratio against that surface is at least 4.5:1

#### Scenario: Load-bearing edges route to border-strong

- **WHEN** a control edge is the sole boundary signal (input-hover border, divider hover, the dashed "none" swatch)
- **THEN** it uses `--border-strong`, which clears at least 3:1 on every neutral plane
- **AND** the decorative `--border`, which is below 3:1 by design, is never relied on as the sole boundary signal

### Requirement: Typography System

The application SHALL adopt Inter Variable as its UI typeface (`--font-ui`) and JetBrains Mono Variable as its monospace typeface (`--font-mono`). Both fonts SHALL be vendored locally as `woff2` files under `src/assets/fonts/` and declared via `@font-face` in `index.html` with `font-display: swap` and a metric-compatible system fallback stack.

Mono SHALL be used as a *type system* across the chrome — not only for code blocks but also for change identifiers, branch names, file paths, and timestamps — so that these elements line up vertically across rows. Task progress is no longer a textual element in the tree row (it renders as a fill meter — see the *Task Progress Meter* requirement), so it is not part of this mono type system.

#### Scenario: Fonts loaded from local assets, not the network

- **WHEN** the application is started offline
- **THEN** Inter and JetBrains Mono render correctly
- **AND** no network request is made for font files

#### Scenario: Mono applies to identifier-like elements in the tree

- **WHEN** a workspace tree row displays a change ID, branch name, or mtime
- **THEN** that element is rendered in `--font-mono`
- **AND** identifier glyphs of equal character count align vertically across rows

### Requirement: Tree Row Selection Model

A selected row in the workspace tree (and in any other list surface that conforms to the row grammar, such as the settings workspaces list) SHALL render a 2px solid `--accent` left bar AND a full-row `--accent-tint` background wash, together with a soft accent glow via `--shadow-accent` (a 1px accent edge plus a low-alpha, wide-blur outward glow capped at 0.35 alpha). The `--text` label SHALL retain at least 4.5:1 on the wash. Inline links rendered on a selected row SHALL use `--accent-hover` rather than `--accent`. Hover over an already-selected row SHALL deepen the wash to `--accent-tint-strong`.

Hover state on an UNSELECTED row SHALL render `background: var(--surface-2)` uniformly on every row, regardless of depth, with no accent bar and no glow — EXCEPT an unselected completed two-line change row, whose green completion wash instead deepens from `--ok-tint` to `--ok-tint-strong` on hover (see the *Completed-State Styling* requirement). That completion-state hover renders a green (not accent) wash and still carries no accent bar and no glow, so it does not borrow selection styling. Keyboard focus SHALL render the `--shadow-focus` recipe (a `--bg`-colored gap, then a 2px `--accent` ring, then an `--accent-glow` halo) via `box-shadow` with `outline: none`, replacing the previous flat outline.

The prior prohibition on an `--accent-tint` selection background is explicitly LIFTED; the wash + bar + glow together are the selection signal. The `--accent-tint` wash remains the exclusive signal of *selection*: whenever a row is both selected and complete, the `--accent-tint` selection wash SHALL override the `--ok-tint` completion wash. Clipping of the outer glow halo by an `overflow: hidden` / `auto` ancestor is acceptable — the 2px accent ring/bar remains visible and AA-compliant.

#### Scenario: Selected row in the tree

- **WHEN** the user clicks an unselected tree row
- **THEN** the row renders a 2px left bar in `--accent`, a full-row `--accent-tint` background wash, and an `--shadow-accent` glow
- **AND** the `--text` label retains at least 4.5:1 contrast on the wash

#### Scenario: Hover does not borrow selection styling

- **WHEN** the user hovers over an unselected tree row that is not a completed change row
- **THEN** the row background is `--surface-2` on every such row, regardless of depth
- **AND** the row renders no accent bar, wash, or glow

#### Scenario: Hover over an unselected completed change row

- **WHEN** the user hovers over an unselected completed two-line change row
- **THEN** the row's `--ok-tint` completion wash deepens to `--ok-tint-strong`, not to `--surface-2`
- **AND** the row renders no `--accent` bar, accent wash, or glow (the green wash does not borrow selection styling)

#### Scenario: Hover over an already-selected row

- **WHEN** the user hovers over a row that is already selected
- **THEN** the wash deepens to `--accent-tint-strong`
- **AND** the body label remains at least 4.5:1 on the deepened wash

#### Scenario: Selection wins over completion on a selected completed row

- **WHEN** a row is both selected and fully complete
- **THEN** the `--accent-tint` selection wash and `--accent` bar are shown, overriding the `--ok-tint` completion wash and `--ok-strong` rail
- **AND** no green wash is visible while the row remains selected

#### Scenario: Keyboard focus uses the shadow-focus recipe

- **WHEN** a tree row (or other row-grammar surface) receives keyboard focus
- **THEN** it renders `box-shadow: var(--shadow-focus)` with `outline: none`
- **AND** the 2px accent ring clears at least 3:1 against `--bg`

### Requirement: Outlined Chip Badges

Status badges (e.g., `DIVERGED`, branch-name labels, change-id labels, count badges) SHALL render as outlined chips with `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase` (where applicable), `letter-spacing: 0.05em`, `font-family: var(--font-mono)`, and `font-size: var(--text-xs)`. The previous tinted-fill pill style MUST NOT be used.

Where horizontal space is tight, a status indicator MAY collapse to a 4px circular dot rendered in the same color (`--warn` for problem states, `--ok` for healthy states). A dot indicator SHALL always carry a `title` attribute with the full label for hover disclosure and accessibility.

Informational chips and status dots SHALL remain outlined / transparent and SHALL NOT carry any `box-shadow` glow or halo. The accent and status glow tokens (`--accent-glow`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`) are reserved EXCLUSIVELY for the selected tree row, the primary button, focused inputs, the focus ring, and the in-progress meter.

The row grammar sanctions exactly **three** filled elements. Two are symmetric — the two task-states of a change: the **task-progress meter** (see the *Task Progress Meter* requirement) is the *in-progress* fill, a fill rendered in `--ok` inside an outlined track, and the **completion mark** (see the *Completed-State Styling* requirement) is the *done* fill, a solid `--ok-strong` disc with a knocked-out check. The third is the **favorite star** (see the *Change-Row Favorite Toggle* requirement in the `spec-browser` capability): a solid star glyph in `--accent` ink marking a favorited change row — a glyph-ink fill like the completion mark. None of these is a chip fill; the selected-row accent wash and the primary-button fill are governed by the *Tree Row Selection Model* and *Accent Color* requirements respectively. No OTHER chip, badge, or status dot SHALL use a filled background. Of the three sanctioned filled elements only the in-progress meter MAY carry a glow (its optional `--glow-ok` halo); the completion mark and the favorite star SHALL carry none, so the reserved-glow set above is unchanged. Outside the row grammar, the markdown view's checked task checkbox (see the *Markdown Task-Checkbox Treatment* requirement) is the document-surface sibling of the completion mark — a sanctioned `--ok-strong` done fill with a knocked-out check, likewise glow-free — so the reserved-glow set above remains unchanged.

Missing-artifact rows are NOT represented as outlined chips; they follow the dim-row treatment defined in the spec-browser capability and the *Dim Row Style for Missing Artifacts* requirement.

#### Scenario: Divergence dot replaces a chip in dense rows

- **WHEN** divergence is indicated in a dense row where a full chip would not fit
- **THEN** a 4px circular dot in `--warn` (for diverged) or another status color is rendered instead
- **AND** the dot's container exposes a `title` attribute carrying the human-readable status label

#### Scenario: Chips and dots carry no glow

- **WHEN** a row renders informational chips (change-id, branch, DIVERGED, count) and status dots
- **THEN** none of them carry a `box-shadow` glow or halo
- **AND** the accent and status glow tokens appear only on the selected row, the primary button, focused inputs, the focus ring, and the in-progress meter

#### Scenario: The meter, completion mark, and favorite star are the only filled row elements

- **WHEN** a tree row renders the task-progress meter or the completion disc or the favorite star alongside outlined chips (change-id, divergence) and status dots
- **THEN** the meter's fill (inside its outlined track), the completion disc, and the favorited row's solid star are the only elements with a filled, non-transparent background within the row grammar
- **AND** every other chip, badge, and dot in the row remains outlined or transparent-backed
- **AND** only the in-progress meter carries an `--ok` glow; the completion disc and the favorite star carry none

### Requirement: Task Progress Meter

Task progress in workspace-tree rows SHALL be rendered as a fixed-width fill meter rather than as a textual `completed/total` count. The meter SHALL consist of an outlined track — `border: var(--border-width) solid var(--border)`, transparent background, `border-radius: var(--radius-sm)`, a fixed inline width (≈56px), and a small fixed block height (≈4–6px) — containing a fill element rendered in `--ok` whose inline width is `completed / total` of the track, clamped to `[0, 1]`. The meter SHALL render no inline digits. In the dark scheme `--ok` is `#34d399` (9.34:1 on `--surface`).

The in-progress fill MAY carry a faint `--glow-ok` halo (`box-shadow: 0 0 8px -1px rgba(52, 211, 153, 0.45)` in dark) so that in-progress work reads as live. This is the single sanctioned `--ok` glow; because the meter is not rendered at `total === 0` or at `completed === total`, the halo is inherently omitted on an empty track and at full completion.

The exact count SHALL be exposed non-visually rather than dropped: the meter SHALL carry `role="progressbar"` with `aria-valuemin` `0`, `aria-valuemax` equal to the total task count, `aria-valuenow` equal to the completed task count, and a `title` (and matching `aria-label`) of the form "N of M tasks".

The meter SHALL NOT be rendered when the change has no parseable tasks (`total === 0`), and SHALL NOT be rendered at full completion (`completed === total`). At full completion the consuming row surfaces its trailing `✓` glyph in place of the meter, so the meter only ever depicts genuinely-in-progress work.

The fill width MAY animate via a CSS `transition` so that a watcher-driven completion change visibly nudges the bar. This transition SHALL be disabled under `@media (prefers-reduced-motion: reduce)`.

#### Scenario: In-progress meter is an outlined track with a green fill

- **WHEN** the meter is rendered for a change with at least one incomplete task
- **THEN** it renders an outlined transparent-background track with an inner fill in `--ok`
- **AND** the fill's inline width is proportional to `completed / total`
- **AND** no digits are rendered inside the meter

#### Scenario: In-progress fill may carry a faint ok glow

- **WHEN** the in-progress meter fill is rendered
- **THEN** it MAY carry the `--glow-ok` halo
- **AND** the halo is omitted at zero tasks and at full completion because the meter is not rendered in those states

#### Scenario: Count is exposed via aria and tooltip, not digits

- **WHEN** the meter is rendered
- **THEN** its element carries `role="progressbar"` with `aria-valuenow` = completed, `aria-valuemax` = total
- **AND** hovering the meter discloses a `title` of the form "N of M tasks"

#### Scenario: Meter is omitted at zero tasks and at full completion

- **WHEN** a change has no parseable tasks (`total === 0`)
- **THEN** no meter is rendered
- **WHEN** a change has every task complete (`completed === total`, `total > 0`)
- **THEN** no meter is rendered and the consuming row shows its trailing `✓` glyph instead

#### Scenario: Reduced motion disables the fill transition

- **WHEN** the OS reports `prefers-reduced-motion: reduce`
- **THEN** the meter's fill width changes without an animated transition

### Requirement: Dim Row Style for Missing Artifacts

When the spec-browser capability indicates that a tree row represents a missing artifact, the row SHALL be rendered with a uniform opacity reduction applied to the entire row contents (label, chevron-spacer, and any other row chrome) rather than via a coloured chip, badge, or replacement icon. The opacity reduction SHALL resolve from a single design token so that the dim treatment is consistent across light and dark schemes and across tinted and untinted rows.

- The dim token SHALL resolve to `0.45` in the default theme.
- The dim treatment SHALL NOT alter the row's foreground colour, font, or layout footprint — only its opacity.
- The dim treatment SHALL compose under hover and selection states: an interactive row that becomes dim simultaneously loses pointer interactivity (per the spec-browser capability), so the hover and selection styles are never observed against a dim row in practice; the visual design need not paint a "dim + hovered" combined state.

#### Scenario: Missing artifact row uses the dim opacity token

- **WHEN** an artifact row is rendered for a missing artifact
- **THEN** the row's computed opacity is `0.45`
- **AND** no foreground colour shift, font change, or layout change is applied relative to a non-missing artifact row at the same depth

#### Scenario: Dim treatment composes consistently across tints

- **WHEN** a missing artifact row appears under a tinted top-level workspace row
- **THEN** the dim treatment is the same opacity reduction as for a missing artifact row under an untinted workspace
- **AND** the tint of the parent top-level row is unaffected (the tint is applied at top level only; the dimmed child row continues to sit on the default child-row background, now muted by the opacity reduction)

### Requirement: Uniform Row Grammar Across List Surfaces

All list-row-like surfaces in the application — including but not limited to the workspace tree row, the settings workspaces list row, and any future archived-changes list — SHALL share a common row template: same vertical padding, same horizontal padding, the same divider treatment between rows, and the same hover/selected states defined by the selection model requirement.

#### Scenario: Settings workspaces row matches tree row grammar

- **WHEN** the settings view renders a registered-workspace row
- **THEN** the row uses the same vertical padding, divider color, hover background, and selection border treatment as a workspace tree row

### Requirement: List-Row Vertical Rhythm Tuned for 4K @ 100%

The vertical padding of list-row-like surfaces SHALL be set so that rows read comfortably on a 4K display at 100% OS scale (one CSS px = one device px) without losing the dense-browser character of the sidebar. Specifically:

- The workspace tree row (`.tree-row`) SHALL use 5px vertical padding (top and bottom).
- The settings workspaces row (`.workspace-row`) SHALL use `--space-4` (16px) vertical padding.
- The settings toggle row (`.settings-toggle-row`) SHALL use 6px vertical padding.

The horizontal padding of these rows is unchanged by this requirement; only vertical rhythm is constrained.

#### Scenario: Tree row breathes at the retuned padding

- **WHEN** a workspace tree row is rendered
- **THEN** the row's computed `padding-top` and `padding-bottom` are both 5px
- **AND** the row's horizontal padding values are unchanged from the existing layout

#### Scenario: Settings workspaces row tracks the tree row

- **WHEN** a registered-workspace row is rendered in settings
- **THEN** the row's computed vertical padding resolves from `--space-4`
- **AND** the row does not feel visually tighter than a sidebar tree row at the same display scale

### Requirement: Inline SVG Icon Set

The application SHALL replace the placeholder text glyphs `▸`, `▾`, `●`, `✕` with hand-rolled inline SVG components exported from `src/components/icons.tsx`. The set SHALL include, at minimum: `ChevronRight`, `ChevronDown`, `Settings`, `Close`, and `Dot` (filled and outlined variants). Icons SHALL accept `width` and `height` props (default 14px), use `currentColor` for `fill` or `stroke`, and use a consistent `stroke-width` of 1.5 for outlined glyphs. No third-party icon library SHALL be added.

#### Scenario: Chevron in tree rows is an SVG

- **WHEN** a tree row with children renders its disclosure indicator
- **THEN** the indicator is an inline `<svg>` from `icons.tsx`
- **AND** no `▸` or `▾` text character appears in the rendered DOM

### Requirement: macOS Hidden Inset Titlebar Layout

On macOS, the main application window SHALL use a hidden / overlay titlebar so that the system traffic lights float over the top-left of the sidebar. The sidebar background on every platform — including macOS — SHALL render `var(--surface)` via the application stylesheet; no platform-specific transparent fallback or operating-system vibrancy effect is applied beneath the sidebar.

The sidebar MAY render a 1px inner right-edge highlight via `box-shadow: var(--sidebar-edge)` (`inset -1px 0 0 0 rgba(255, 255, 255, 0.03)` in dark, `rgba(0, 0, 0, 0.04)` in light) so that the solid sidebar reads as the front-most plane against the darker detail pane. This is a `box-shadow` on the same solid `--surface` element and introduces NO `NSVisualEffectView` / window-vibrancy material; the solid-background lock is unchanged.

The top of the sidebar SHALL reserve `--space-6` (32px) of safe-area padding on macOS so that traffic-light buttons do not overlap interactive content. The application SHALL provide an explicit drag region across the top 32px of the window on macOS so that the hidden inset titlebar remains draggable; the drag region MAY be either a `data-tauri-drag-region` element or an explicit `getCurrentWindow().startDragging()` call wired to mousedown. The `core:window:allow-start-dragging` permission SHALL be present in the Tauri capabilities ACL so the IPC drag call is allowed.

This layout is a property of the **native desktop window** and applies only there. Whether the frontend is running inside that native window SHALL be determined from the host itself and SHALL NOT be inferred from the browser user-agent string; only once the native host is established may the operating system be distinguished by any means available. The served web UI SHALL NOT reserve the traffic-light safe-area padding and SHALL NOT render the titlebar drag region on any platform or device — including a browser running on macOS, and including a browser whose user-agent reports a `Macintosh` or `Mac OS X` token while running on a mobile or tablet operating system. Because the drag region is an interaction surface layered over the top of the window, rendering it outside the native window would intercept input intended for the content beneath it; the served web UI SHALL therefore leave that area free.

On Windows and Linux, the operating system's default titlebar SHALL be used. The sidebar background SHALL be `var(--surface)`, matching macOS.

#### Scenario: macOS sidebar renders a solid surface background

- **WHEN** the application launches on macOS
- **THEN** the sidebar element's computed background resolves to `var(--surface)`
- **AND** no `NSVisualEffectView` / `window-vibrancy` material is applied to the main window
- **AND** the sidebar MAY carry the `--sidebar-edge` inner box-shadow highlight, which is not a vibrancy material
- **AND** the traffic-light buttons still appear inset over the sidebar's top-left

#### Scenario: macOS sidebar reserves traffic-light safe area

- **WHEN** the application launches on macOS
- **THEN** the `.split-pane-left` element has top padding of `--space-6` (32px)
- **AND** the first sidebar row clears the traffic-light buttons

#### Scenario: Window draggable from the titlebar strip on macOS

- **WHEN** the user presses and holds the primary mouse button anywhere in the top 32px of the window on macOS, outside the settings-toggle button
- **THEN** the window enters native drag mode
- **AND** moving the mouse moves the window

#### Scenario: Served web UI on macOS reserves no titlebar chrome

- **WHEN** the served web UI is loaded in a browser running on macOS
- **THEN** the side panes reserve no traffic-light safe-area padding
- **AND** no titlebar drag region is rendered over the top of the page
- **AND** the full width of the top of the detail pane accepts input

#### Scenario: A Mac-like mobile user-agent does not enable desktop titlebar chrome

- **WHEN** the served web UI is loaded in a browser whose user-agent contains a `Macintosh` or `Mac OS X` token but which is not the native desktop window
- **THEN** the side panes reserve no traffic-light safe-area padding
- **AND** no titlebar drag region is rendered
- **AND** no vertical space is consumed for window controls that do not exist

#### Scenario: Windows and Linux render solid chrome

- **WHEN** the application launches on Windows or Linux
- **THEN** the sidebar background is `var(--surface)`
- **AND** the operating system's default titlebar is used

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` as unboxed colour-tinted text (a dedicated `--code-fg` colour at `font-weight: 500`, with no fill, border, or radius), and fenced code blocks in `--font-mono` with `--leading-code` (1.5). The maximum content width SHALL be 880px — chosen so that body prose at `--text-lg` renders roughly 100 characters per line and fenced code at `--text-md` renders roughly 97 characters per line.

Inline `<code>` (a `<code>` element not inside a `<pre>`) SHALL render as unboxed text — no background fill, no border, and no border-radius — distinguished from body prose by `--font-mono`, a `font-weight` of 500, and a dedicated `--code-fg` colour token. `--code-fg` SHALL be a hue distinct from `--accent` (which colours markdown links), so inline code and links remain separable even where they sit together, with the mono family reinforcing the distinction. `--code-fg` SHALL be defined per scheme and SHALL clear the AA 4.5:1 contrast floor against the markdown background in both schemes — a darker shade on light, a brighter shade on dark. The same unboxed-text recipe SHALL be used for `.settings-help code`, so the application has ONE inline-code recipe.

Fenced code blocks (`pre`) SHALL render as a lifted well: `--surface` background, 1px `--border`, `--radius`, and `--shadow-2` (which includes the inner top-light), distinct from the unboxed inline code. The `pre code` element SHALL remain transparent and borderless. Blockquotes SHALL render as `--surface-3` aside cards with a 3px accent-at-0.7 left rule and `--text-muted` body.

Markdown-rendering treatments beyond typography, the elevation/aside treatments described here, and the task-checkbox treatment (see the *Markdown Task-Checkbox Treatment* requirement) — callouts, anchor links, custom code-block chrome — remain out of scope of this requirement and are not sanctioned by it.

#### Scenario: Body text uses Inter at the prose size

- **WHEN** the detail pane renders any markdown paragraph
- **THEN** the paragraph computed `font-family` is `--font-ui`
- **AND** the computed `font-size` is `--text-lg`
- **AND** the computed `line-height` is `--leading-prose`

#### Scenario: Inline code is unboxed coloured text

- **WHEN** the detail pane renders a `<code>` element that is not inside a `<pre>`
- **THEN** the element has no background fill, no border, and no border-radius
- **AND** the element font-family is `--font-mono`
- **AND** the element font-weight is 500
- **AND** the element colour is `--code-fg`, a hue distinct from `--accent`

#### Scenario: Fenced code block is a lifted well

- **WHEN** the detail pane renders a `<pre>` fenced code block
- **THEN** it renders with a `--surface` background, a 1px `--border`, and `--shadow-2`
- **AND** the inner `pre code` element is transparent and borderless

### Requirement: Theme Follows System Preference

The application SHALL follow the operating system's `prefers-color-scheme` for light/dark theming and SHALL NOT expose an in-app theme override toggle. `:root` SHALL declare `color-scheme: light dark` so that native scrollbars and form controls also adapt to the system theme.

#### Scenario: System dark mode switches the app

- **WHEN** the operating system theme switches from light to dark while the app is running
- **THEN** the application updates its neutral tokens to the dark scheme
- **AND** the accent and status tokens remain unchanged

#### Scenario: No theme toggle in settings

- **WHEN** the user opens the Settings view
- **THEN** no control to override the OS theme is presented

### Requirement: Flat Tree Row Geometry

The workspace tree row (`.tree-row`) SHALL render without a `border-radius` and without an inline-axis margin (no side gutter between the row and the sidebar edge). The row's tint background, hover background, and selection left bar SHALL therefore fill the row edge-to-edge across the full sidebar width.

This geometry SHALL apply uniformly to every tree row regardless of depth or tint state, so tinted top-level rows and untinted child rows share the same horizontal footprint and the row grammar remains uniform across the tree.

The existing 2px inline-start transparent border (used to reserve space for the selection bar so selected and unselected rows do not shift horizontally) SHALL be preserved — only the corner radius and outer inline margin are removed.

#### Scenario: Tree row renders edge-to-edge

- **WHEN** a workspace tree row is rendered
- **THEN** the row's computed `border-radius` is `0`
- **AND** the row's computed inline-axis margin (left and right) is `0`
- **AND** the row's tint or hover background extends from the sidebar's inline-start edge to its inline-end edge without any gutter

#### Scenario: Tinted and untinted rows share row geometry

- **WHEN** a tinted top-level workspace row is rendered above an untinted child row
- **THEN** both rows resolve to the same `border-radius` (`0`) and the same inline margin (`0`)
- **AND** the visible difference between them is only the tint background on the parent, not the row footprint

### Requirement: Completed-State Styling

The workspace tree SHALL render completion in the success-green family rather than in a muted or dimmed neutral, so that a finished item reads as complete at a glance. This applies at three levels — the milestone completion glyph, a completed change's rail and background wash, and a completed leaf task — and at every level a colour-independent shape carries the "done" meaning while green provides reinforcement (so the signal survives colour-vision deficiency).

A dedicated foreground token `--ok-strong` SHALL provide the "done" green for the foreground marks, distinct from `--ok` (which is tuned as the fill inside the outlined task-progress meter). `--ok` used as a foreground on the light scheme's white `--surface` is only ~2.6:1 — below AA for text and below the 3:1 non-text floor — so `--ok-strong` SHALL be a deeper green: `#047857` on light (≥4.5:1 on `--surface`) and `#34d399` on dark (9.34:1 on `--surface`). `--ok` retains its role as the progress-meter fill and as the source hue for the completion wash tokens.

A token pair `--ok-tint` / `--ok-tint-strong` SHALL provide the completed-change background wash and its hover-deepened variant, mirroring the `--accent-tint` / `--accent-tint-strong` selection-wash pair: `--ok-tint` SHALL use the `--accent-tint` alpha and `--ok-tint-strong` the `--accent-tint-strong` alpha over the `--ok` hue, in both schemes, so text contrast on the completion wash is no worse than on the already-accepted selection wash. These wash tokens SHALL derive from `--ok` (the bright green), not from `--ok-strong` (a foreground colour).

**Milestone completion glyph.** When a Section, the Tasks artifact node, or a whole change/instance is fully complete, the trailing completion glyph SHALL render as a solid `--ok-strong` disc with a knocked-out check (a check in `--surface`, punched through to the surface plane), NOT as a muted outline checkmark. This disc is the *done* fill sanctioned by the *Outlined Chip Badges* requirement. It SHALL carry no `box-shadow` glow or halo. It SHALL be visually distinct from a 4px status dot — larger (on the order of 15px) and carrying an interior check — so it is not mistaken for a status dot.

**Completed-change rail and wash.** A completed two-line change row SHALL render its left rail in `--ok-strong`, replacing the workspace-palette-colour rail (`tree-row--rail-{color}`) it would otherwise show, AND SHALL render a soft `--ok-tint` background wash across the whole row. The rail and wash are **additive** reinforcement layered on the milestone completion disc: the disc (a colour-independent shape) remains the primary "done" signal and SHALL NOT be removed or replaced by the wash. Selection SHALL still win: when a completed change row is selected, the `--accent` selection bar AND the `--accent-tint` selection wash defined by the *Tree Row Selection Model* requirement SHALL override BOTH the completion rail and the completion wash, so the green wash appears only on an unselected completed row. A hover over an unselected completed change row SHALL deepen the wash from `--ok-tint` to `--ok-tint-strong` rather than revert to the neutral `--surface-2` hover background. The `--accent-tint` wash remains the exclusive signal of *selection*; the `--ok-tint` completion wash is a distinct, lower-priority state that yields to it whenever both apply.

The completion wash SHALL apply to the two-line change row only. Sections, the Tasks artifact node, multi-instance child rows, and leaf task rows SHALL NOT receive the change-row background wash; their completion continues to be signalled by the disc (Section / Tasks node / multi-instance child) or by green struck text (leaf task).

**Completed leaf task.** A completed leaf task row SHALL render its label in `--ok-strong` and SHALL retain its `text-decoration: line-through`. The line-through is the colour-independent "done" signal for a leaf task (which carries no glyph); the green is reinforcement. A completed leaf task SHALL NOT receive the filled completion disc, and SHALL NOT receive the change-row background wash — the disc and the wash are reserved for milestone completion (Section / change), keeping the atom lighter than the milestone.

The foreground green `--ok-strong` covers the disc fill, the rail, and the completed-task label; the disc's knocked-out check resolves from `--surface`; the completed-change background wash uses `--ok-tint` (`--ok-tint-strong` on hover). `--ok-strong` clears AA for the completed-task text in both schemes, and `--text` on the `--ok-tint` wash clears at least 4.5:1 in both schemes.

#### Scenario: Milestone completion glyph is a filled ok disc

- **WHEN** a Section, the Tasks artifact node, or a whole change is fully complete and its trailing completion glyph is rendered
- **THEN** the glyph is a solid `--ok-strong` disc with a knocked-out `--surface` check, not a muted outline checkmark
- **AND** the disc carries no glow
- **AND** the disc is larger than, and distinguishable from, a 4px status dot

#### Scenario: Completed change shows an ok rail and green wash

- **WHEN** an unselected two-line change row is fully complete
- **THEN** its left rail renders in `--ok-strong` rather than in the workspace palette colour
- **AND** a soft `--ok-tint` background wash is rendered across the whole row
- **AND** the milestone completion disc remains rendered (the wash is additive, not a replacement)

#### Scenario: Hover deepens the completion wash rather than clearing it

- **WHEN** the user hovers over an unselected completed two-line change row
- **THEN** the background wash deepens from `--ok-tint` to `--ok-tint-strong`
- **AND** the row does not revert to the neutral `--surface-2` hover background
- **AND** the row renders no `--accent` bar, accent wash, or glow

#### Scenario: Selection overrides the completed-change rail and wash

- **WHEN** a fully-complete change row is selected
- **THEN** the `--accent` selection bar and `--accent-tint` selection wash are shown, overriding BOTH the `--ok-strong` completion rail and the `--ok-tint` completion wash
- **AND** when the row is deselected the `--ok-strong` rail and `--ok-tint` wash return

#### Scenario: Completion wash is confined to the change row

- **WHEN** a fully-complete change's Sections, Tasks artifact node, multi-instance child rows, and leaf tasks are rendered
- **THEN** none of them receive the `--ok-tint` change-row background wash
- **AND** their completion is signalled by the completion disc or by green struck text, as before

#### Scenario: Completed leaf task is green and struck

- **WHEN** a leaf task is complete
- **THEN** its label renders in `--ok-strong` with `text-decoration: line-through`
- **AND** it does not render the filled completion disc
- **AND** it does not receive the change-row background wash

#### Scenario: Done state reads without colour

- **WHEN** any completed item is rendered
- **THEN** a colour-independent shape signals completion — the check inside the disc for a milestone, the line-through for a leaf task
- **AND** the green (`--ok-strong` foreground, `--ok-tint` wash) reinforces but is not the sole carrier of the completion meaning

### Requirement: Markdown Task-Checkbox Treatment

The markdown view SHALL render GFM task-list checkboxes as inline SVG glyphs from the application icon set, NOT as native `<input type="checkbox">` controls. The treatment SHALL apply on every surface that renders markdown through the shared `.markdown-view` renderer (the detail pane, the archive reading view, and the file-browser preview).

The glyphs SHALL render at 16px so they sit flush with the `--text-lg` markdown body text.

A checked task (`- [x]`) SHALL render as a solid `--ok-strong` rounded square with its check knocked out in `--bg` (the plane every markdown surface sits on) — the same knocked-out-check construction as the tree's completion mark (identical check geometry, stroke-width 2.5, round caps and joins in the 24×24 viewBox), squared off, so the two marks read as siblings. The checked glyph SHALL carry no `box-shadow` glow or halo. An unchecked task (`- [ ]`) SHALL render as an outlined square stroked in `--text-faint` with no fill. The pending box is a meaningful status boundary, so its ink SHALL clear the 3:1 non-text floor on `--bg` in both schemes (`--text-faint`: 4.70:1 light / 5.43:1 dark); the light scheme's `--border-strong` (1.53:1 on `--bg`) is decorative-grade there and MUST NOT carry this boundary.

The checkbox is a STATUS glyph, not a control: it SHALL NOT use the accent family, preserving the accent-fill discipline of the *Accent Color* requirement. The checked fill SHALL be `--ok-strong` (the AA-clearing foreground "done" green), NOT `--ok` (which stays reserved as the in-progress meter fill).

A checked task's line text SHALL render in `--text-faint` with NO line-through — on this surface the glyph is the colour-independent "done" signal (unlike the tree's leaf-task rows, which carry no glyph and strike their labels). The dimming cascades to nested content of the checked task, EXCEPT that a pending (unchecked) task line nested under a checked task SHALL keep the default `--text` colour.

The glyphs SHALL remain inert — satisfying the spec-browser *Read-Only Viewer* requirement structurally, with no click behaviour to suppress — and SHALL expose checkbox semantics to assistive technology: an element with `role="checkbox"`, `aria-checked` reflecting the task state, and `aria-disabled="true"`, without introducing a keyboard focus stop per task line.

The settings view's toggle rows are real interactive controls, not markdown status glyphs; they SHALL retain native checkbox rendering with `accent-color: var(--accent)`.

#### Scenario: Checked task renders the filled done glyph

- **WHEN** the detail pane renders a `tasks.md` line `- [x] <label>`
- **THEN** the line's leading glyph is a 16px solid `--ok-strong` rounded square with a `--bg` knocked-out check matching the completion mark's check construction
- **AND** no native `<input>` element is rendered for the task line
- **AND** the glyph carries no glow

#### Scenario: Unchecked task renders the outlined pending glyph

- **WHEN** the detail pane renders a `tasks.md` line `- [ ] <label>`
- **THEN** the line's leading glyph is a 16px outlined square stroked in `--text-faint` with no fill
- **AND** the stroke ink clears 3:1 against `--bg` in both colour schemes

#### Scenario: Checked line text dims without strikethrough

- **WHEN** a task line is checked
- **THEN** its line text renders in `--text-faint`
- **AND** it carries no line-through decoration

#### Scenario: Pending subtask under a checked parent keeps full-strength text

- **WHEN** an unchecked task line is nested under a checked task line
- **THEN** the nested pending line's text renders in the default `--text` colour

#### Scenario: Checkbox state reaches assistive technology without a focus stop

- **WHEN** a task checkbox glyph is rendered
- **THEN** it exposes `role="checkbox"` with `aria-checked` matching the task state and `aria-disabled="true"`
- **AND** it is not keyboard-focusable

#### Scenario: Settings toggle keeps its native control rendering

- **WHEN** the settings view renders a toggle row (for example notifications)
- **THEN** it renders a native checkbox with `accent-color: var(--accent)`, unaffected by the markdown glyph treatment

#### Scenario: Treatment applies on every markdown surface

- **WHEN** the archive reading view or the file-browser preview renders markdown containing task lines
- **THEN** the task checkboxes render with the same glyph treatment as the detail pane

