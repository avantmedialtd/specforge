## ADDED Requirements

### Requirement: Markdown Task-Checkbox Treatment

The markdown view SHALL render GFM task-list checkboxes as inline SVG glyphs from the application icon set, NOT as native `<input type="checkbox">` controls. The treatment SHALL apply on every surface that renders markdown through the shared `.markdown-view` renderer (the detail pane, the archive reading view, and the file-browser preview).

The glyphs SHALL render at 16px so they sit flush with the `--text-lg` markdown body text.

A checked task (`- [x]`) SHALL render as a solid `--ok-strong` rounded square with its check knocked out in `--bg` (the plane every markdown surface sits on) — the same knocked-out-check construction as the tree's completion mark (identical check geometry, stroke-width 2.5, round caps and joins in the 24×24 viewBox), squared off, so the two marks read as siblings. The checked glyph SHALL carry no `box-shadow` glow or halo. An unchecked task (`- [ ]`) SHALL render as an outlined square stroked in `--border-strong` (the load-bearing control edge) with no fill.

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
- **THEN** the line's leading glyph is a 16px outlined square stroked in `--border-strong` with no fill

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

## MODIFIED Requirements

### Requirement: Accent Color

The application SHALL use a vivid indigo accent system in the indigo/violet family. The raw macOS system blue (`rgb(0, 122, 255)`) MUST NOT appear in user-visible chrome.

In the dark scheme the accent tokens SHALL be: `--accent` `#7c8cff` (the ink/line color used for links, the selection bar, focus rings, syntax-highlight titles, and the settings-toggle checkbox `accent-color` — markdown task checkboxes are status glyphs in the ok family per the *Markdown Task-Checkbox Treatment* requirement, not accent-coloured controls); `--accent-hover` `#93a1ff` (hover BRIGHTENS on dark); `--accent-active` `#5d6ef0` (pressed); `--accent-strong` `#4f5fe0` (the fill-under-white-text surface, used ONLY as the primary-button background, where the white label SHALL reach at least 4.5:1 — `#4f5fe0` yields 5.19:1); `--accent-tint` `rgba(124, 140, 255, 0.14)`; `--accent-tint-strong` `rgba(124, 140, 255, 0.22)`; `--accent-glow` `rgba(124, 140, 255, 0.35)`.

In the light scheme the accent SHALL hold its indigo identity with the INVERSE hover direction (on a light background, lift = darken): `--accent` `#4f5bd9`, `--accent-hover` `#3f4bc4`, `--accent-active` `#3a46b8`.

The accent SHALL appear FILLED or GLOWING in exactly three places — the selected tree row, the primary button, and the in-progress task-progress meter — plus focused inputs and the focus ring. Everywhere else (links, informational chips, status dots) the accent or status color SHALL be ink/outline only and SHALL carry no fill or glow. The sanctioned *done* fills — the completion mark in the row grammar and the checked markdown task checkbox in the document surface (see the *Outlined Chip Badges* and *Markdown Task-Checkbox Treatment* requirements) — are the only status-colour fills beyond the in-progress meter's sanctioned `--ok` fill (see the *Task Progress Meter* requirement), and both carry no glow.

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

### Requirement: Outlined Chip Badges

Status badges (e.g., `DIVERGED`, branch-name labels, change-id labels, count badges) SHALL render as outlined chips with `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase` (where applicable), `letter-spacing: 0.05em`, `font-family: var(--font-mono)`, and `font-size: var(--text-xs)`. The previous tinted-fill pill style MUST NOT be used.

Where horizontal space is tight, a status indicator MAY collapse to a 4px circular dot rendered in the same color (`--warn` for problem states, `--ok` for healthy states). A dot indicator SHALL always carry a `title` attribute with the full label for hover disclosure and accessibility.

Informational chips and status dots SHALL remain outlined / transparent and SHALL NOT carry any `box-shadow` glow or halo. The accent and status glow tokens (`--accent-glow`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`) are reserved EXCLUSIVELY for the selected tree row, the primary button, focused inputs, the focus ring, and the in-progress meter.

The row grammar sanctions exactly **two** filled elements, and they are symmetric — the two task-states of a change. The **task-progress meter** (see the *Task Progress Meter* requirement) is the *in-progress* fill: a fill rendered in `--ok` inside an outlined track. The **completion mark** (see the *Completed-State Styling* requirement) is the *done* fill: a solid `--ok-strong` disc with a knocked-out check. Neither is a chip fill; the selected-row accent wash and the primary-button fill are governed by the *Tree Row Selection Model* and *Accent Color* requirements respectively. No OTHER chip, badge, or status dot SHALL use a filled background. Of the two sanctioned filled elements only the in-progress meter MAY carry a glow (its optional `--glow-ok` halo); the completion mark SHALL carry none, so the reserved-glow set above is unchanged. Outside the row grammar, the markdown view's checked task checkbox (see the *Markdown Task-Checkbox Treatment* requirement) is the document-surface sibling of the completion mark — a sanctioned `--ok-strong` done fill with a knocked-out check, likewise glow-free — so the reserved-glow set above remains unchanged.

Missing-artifact rows are NOT represented as outlined chips; they follow the dim-row treatment defined in the spec-browser capability and the *Dim Row Style for Missing Artifacts* requirement.

#### Scenario: Divergence dot replaces a chip in dense rows

- **WHEN** divergence is indicated in a dense row where a full chip would not fit
- **THEN** a 4px circular dot in `--warn` (for diverged) or another status color is rendered instead
- **AND** the dot's container exposes a `title` attribute carrying the human-readable status label

#### Scenario: Chips and dots carry no glow

- **WHEN** a row renders informational chips (change-id, branch, DIVERGED, count) and status dots
- **THEN** none of them carry a `box-shadow` glow or halo
- **AND** the accent and status glow tokens appear only on the selected row, the primary button, focused inputs, the focus ring, and the in-progress meter

#### Scenario: The meter and completion mark are the only filled row elements

- **WHEN** a tree row renders the task-progress meter or the completion disc alongside outlined chips (change-id, divergence) and status dots
- **THEN** the meter's fill (inside its outlined track) and the completion disc are the only elements with a filled, non-transparent background within the row grammar
- **AND** every other chip, badge, and dot in the row remains outlined or transparent-backed
- **AND** only the in-progress meter carries an `--ok` glow; the completion disc carries none

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
