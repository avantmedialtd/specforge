# visual-identity Delta: Favorite Changes in the Workspace Tree

## MODIFIED Requirements

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
