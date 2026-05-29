## MODIFIED Requirements

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

In the dark scheme the accent tokens SHALL be: `--accent` `#7c8cff` (the ink/line color used for links, the selection bar, focus rings, syntax-highlight titles, and the checkbox `accent-color`); `--accent-hover` `#93a1ff` (hover BRIGHTENS on dark); `--accent-active` `#5d6ef0` (pressed); `--accent-strong` `#4f5fe0` (the fill-under-white-text surface, used ONLY as the primary-button background, where the white label SHALL reach at least 4.5:1 — `#4f5fe0` yields 5.19:1); `--accent-tint` `rgba(124, 140, 255, 0.14)`; `--accent-tint-strong` `rgba(124, 140, 255, 0.22)`; `--accent-glow` `rgba(124, 140, 255, 0.35)`.

In the light scheme the accent SHALL hold its indigo identity with the INVERSE hover direction (on a light background, lift = darken): `--accent` `#4f5bd9`, `--accent-hover` `#3f4bc4`, `--accent-active` `#3a46b8`.

The accent SHALL appear FILLED or GLOWING in exactly three places — the selected tree row, the primary button, and the in-progress task-progress meter — plus focused inputs and the focus ring. Everywhere else (links, informational chips, status dots) the accent or status color SHALL be ink/outline only and SHALL carry no fill or glow.

#### Scenario: Accent stays in the indigo family

- **WHEN** the application stylesheet is inspected
- **THEN** the `--accent` family resolves to an indigo/violet hue (approximately hue 231)
- **AND** the raw macOS system blue `rgb(0, 122, 255)` does not appear in any user-visible chrome

#### Scenario: Accent is filled or glowing in exactly three places

- **WHEN** the UI is rendered
- **THEN** an accent fill or glow appears only on the selected tree row, the primary button, the in-progress meter, focused inputs, and the focus ring
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

In the light scheme `--text-faint` darkens to `#6f7a86` (approximately 4.7:1) and `--surface-3` is `#edf0f5`; the other light neutrals are unchanged.

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

### Requirement: Tree Row Selection Model

A selected row in the workspace tree (and in any other list surface that conforms to the row grammar, such as the settings workspaces list) SHALL render a 2px solid `--accent` left bar AND a full-row `--accent-tint` background wash, together with a soft accent glow via `--shadow-accent` (a 1px accent edge plus a low-alpha, wide-blur outward glow capped at 0.35 alpha). The `--text` label SHALL retain at least 4.5:1 on the wash. Inline links rendered on a selected row SHALL use `--accent-hover` rather than `--accent`. Hover over an already-selected row SHALL deepen the wash to `--accent-tint-strong`.

Hover state on an UNSELECTED row SHALL render `background: var(--surface-2)` uniformly on every row, regardless of depth, with no accent bar and no glow. Keyboard focus SHALL render the `--shadow-focus` recipe (a `--bg`-colored gap, then a 2px `--accent` ring, then an `--accent-glow` halo) via `box-shadow` with `outline: none`, replacing the previous flat outline.

The prior prohibition on an `--accent-tint` selection background is explicitly LIFTED; the wash + bar + glow together are the selection signal. Clipping of the outer glow halo by an `overflow: hidden` / `auto` ancestor is acceptable — the 2px accent ring/bar remains visible and AA-compliant.

#### Scenario: Selected row in the tree

- **WHEN** the user clicks an unselected tree row
- **THEN** the row renders a 2px left bar in `--accent`, a full-row `--accent-tint` background wash, and an `--shadow-accent` glow
- **AND** the `--text` label retains at least 4.5:1 contrast on the wash

#### Scenario: Hover does not borrow selection styling

- **WHEN** the user hovers over an unselected tree row
- **THEN** the row background is `--surface-2` on every row, regardless of depth
- **AND** the row renders no accent bar, wash, or glow

#### Scenario: Hover over an already-selected row

- **WHEN** the user hovers over a row that is already selected
- **THEN** the wash deepens to `--accent-tint-strong`
- **AND** the body label remains at least 4.5:1 on the deepened wash

#### Scenario: Keyboard focus uses the shadow-focus recipe

- **WHEN** a tree row (or other row-grammar surface) receives keyboard focus
- **THEN** it renders `box-shadow: var(--shadow-focus)` with `outline: none`
- **AND** the 2px accent ring clears at least 3:1 against `--bg`

### Requirement: Outlined Chip Badges

Status badges (e.g., `DIVERGED`, branch-name labels, change-id labels, count badges) SHALL render as outlined chips with `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase` (where applicable), `letter-spacing: 0.05em`, `font-family: var(--font-mono)`, and `font-size: var(--text-xs)`. The previous tinted-fill pill style MUST NOT be used.

Where horizontal space is tight, a status indicator MAY collapse to a 4px circular dot rendered in the same color (`--warn` for problem states, `--ok` for healthy states). A dot indicator SHALL always carry a `title` attribute with the full label for hover disclosure and accessibility.

Informational chips and status dots SHALL remain outlined / transparent and SHALL NOT carry any `box-shadow` glow or halo. The accent and status glow tokens (`--accent-glow`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`) are reserved EXCLUSIVELY for the selected tree row, the primary button, focused inputs, the focus ring, and the in-progress meter.

The task-progress meter (see the *Task Progress Meter* requirement) is the single sanctioned filled element in the row grammar: it renders a fill inside an outlined track. The selected-row accent wash and the primary-button fill are governed by the *Tree Row Selection Model* and *Accent Color* requirements respectively and are not chip fills. No other chip, badge, or status dot SHALL use a filled background or a glow.

Missing-artifact rows are NOT represented as outlined chips; they follow the dim-row treatment defined in the spec-browser capability and the *Dim Row Style for Missing Artifacts* requirement.

#### Scenario: Divergence dot replaces a chip in dense rows

- **WHEN** divergence is indicated in a dense row where a full chip would not fit
- **THEN** a 4px circular dot in `--warn` (for diverged) or another status color is rendered instead
- **AND** the dot's container exposes a `title` attribute carrying the human-readable status label

#### Scenario: Chips and dots carry no glow

- **WHEN** a row renders informational chips (change-id, branch, DIVERGED, count) and status dots
- **THEN** none of them carry a `box-shadow` glow or halo
- **AND** the accent and status glow tokens appear only on the selected row, the primary button, focused inputs, the focus ring, and the in-progress meter

#### Scenario: Progress meter is the only filled row element

- **WHEN** a tree row renders the task-progress meter alongside outlined chips (change-id, divergence) and status dots
- **THEN** the meter is the only element with a filled (non-transparent) background within the row grammar, and that fill is contained inside its outlined track
- **AND** every other chip, badge, and dot in the row remains outlined or transparent-backed

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

### Requirement: macOS Hidden Inset Titlebar Layout

On macOS, the main application window SHALL use a hidden / overlay titlebar so that the system traffic lights float over the top-left of the sidebar. The sidebar background on every platform — including macOS — SHALL render `var(--surface)` via the application stylesheet; no platform-specific transparent fallback or operating-system vibrancy effect is applied beneath the sidebar.

The sidebar MAY render a 1px inner right-edge highlight via `box-shadow: var(--sidebar-edge)` (`inset -1px 0 0 0 rgba(255, 255, 255, 0.03)` in dark, `rgba(0, 0, 0, 0.04)` in light) so that the solid sidebar reads as the front-most plane against the darker detail pane. This is a `box-shadow` on the same solid `--surface` element and introduces NO `NSVisualEffectView` / window-vibrancy material; the solid-background lock is unchanged.

The top of the sidebar SHALL reserve `--space-6` (32px) of safe-area padding on macOS so that traffic-light buttons do not overlap interactive content. The application SHALL provide an explicit drag region across the top 32px of the window on macOS so that the hidden inset titlebar remains draggable; the drag region MAY be either a `data-tauri-drag-region` element or an explicit `getCurrentWindow().startDragging()` call wired to mousedown. The `core:window:allow-start-dragging` permission SHALL be present in the Tauri capabilities ACL so the IPC drag call is allowed.

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

#### Scenario: Windows and Linux render solid chrome

- **WHEN** the application launches on Windows or Linux
- **THEN** the sidebar background is `var(--surface)`
- **AND** the operating system's default titlebar is used

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` with the outlined-chip treatment (`border: 1px solid var(--border)`, transparent background, `--radius-sm`), and fenced code blocks in `--font-mono` with `--leading-code` (1.5). The maximum content width SHALL be 880px — chosen so that body prose at `--text-lg` renders roughly 100 characters per line and fenced code at `--text-md` renders roughly 97 characters per line.

Inline `<code>` SHALL remain an outlined chip with a 1px `--border` and a TRANSPARENT background — it SHALL NOT take a `--surface-2` fill. The same transparent-chip recipe SHALL be used for `.settings-help code`, so the application has ONE inline-code recipe.

Fenced code blocks (`pre`) SHALL render as a lifted well: `--surface` background, 1px `--border`, `--radius`, and `--shadow-2` (which includes the inner top-light), distinct from the flat transparent inline chip. The `pre code` element SHALL remain transparent and borderless. Blockquotes SHALL render as `--surface-3` aside cards with a 3px accent-at-0.7 left rule and `--text-muted` body.

Markdown-rendering changes beyond typography and the elevation/aside treatments described here (callouts, anchor links, custom code-block chrome) are explicitly out of scope and MUST NOT be introduced by this change.

#### Scenario: Body text uses Inter at the prose size

- **WHEN** the detail pane renders any markdown paragraph
- **THEN** the paragraph computed `font-family` is `--font-ui`
- **AND** the computed `font-size` is `--text-lg`
- **AND** the computed `line-height` is `--leading-prose`

#### Scenario: Inline code is an outlined chip

- **WHEN** the detail pane renders a `<code>` element that is not inside a `<pre>`
- **THEN** the element renders with a 1px `--border` outline
- **AND** the element background is transparent
- **AND** the element font-family is `--font-mono`

#### Scenario: Fenced code block is a lifted well

- **WHEN** the detail pane renders a `<pre>` fenced code block
- **THEN** it renders with a `--surface` background, a 1px `--border`, and `--shadow-2`
- **AND** the inner `pre code` element is transparent and borderless
