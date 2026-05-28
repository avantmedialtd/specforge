## MODIFIED Requirements

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

### Requirement: Outlined Chip Badges

Status badges (e.g., `DIVERGED`, branch-name labels, change-id labels) SHALL render as outlined chips with `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase`, `letter-spacing: 0.05em`, `font-family: var(--font-mono)`, and `font-size: var(--text-xs)`. The previous tinted-fill pill style for `row-divergence-*` MUST NOT be used.

Where horizontal space is tight, a status indicator MAY collapse to a 4px circular dot rendered in the same color (`--warn` for problem states, `--ok` for healthy states). A dot indicator SHALL always carry a `title` attribute with the full label for hover disclosure and accessibility.

The task-progress meter (see the *Task Progress Meter* requirement) is the single sanctioned exception to the "outlined, never filled" vocabulary: it renders a fill *inside* an outlined track. No other chip, badge, or status indicator SHALL use a filled background.

Missing-artifact rows are NOT represented as outlined chips; they follow the dim-row treatment defined in the spec-browser capability (see *Artifact Row Presence Treatment*) and the *Dim Row Style for Missing Artifacts* requirement below.

#### Scenario: Divergence dot replaces a chip in dense rows

- **WHEN** divergence is indicated in a dense row where a full chip would not fit
- **THEN** a 4px circular dot in `--warn` (for diverged) or another status color is rendered instead
- **AND** the dot's container exposes a `title` attribute carrying the human-readable status label

#### Scenario: Progress meter is the only filled row element

- **WHEN** a tree row renders the task-progress meter alongside outlined chips (change-id, divergence) and status dots
- **THEN** the meter is the only element with a filled (non-transparent) background, and that fill is contained inside its outlined track
- **AND** every other chip, badge, and dot in the row remains outlined or transparent-backed

## ADDED Requirements

### Requirement: Task Progress Meter

Task progress in workspace-tree rows SHALL be rendered as a fixed-width fill meter rather than as a textual `completed/total` count. The meter SHALL consist of an outlined track — `border: var(--border-width) solid var(--border)`, transparent background, `border-radius: var(--radius-sm)`, a fixed inline width (≈56px), and a small fixed block height (≈4–6px) — containing a fill element rendered in `--ok` whose inline width is `completed / total` of the track, clamped to `[0, 1]`. The meter SHALL render no inline digits.

The exact count SHALL be exposed non-visually rather than dropped: the meter SHALL carry `role="progressbar"` with `aria-valuemin` `0`, `aria-valuemax` equal to the total task count, `aria-valuenow` equal to the completed task count, and a `title` (and matching `aria-label`) of the form "N of M tasks".

The meter SHALL NOT be rendered when the change has no parseable tasks (`total === 0`), and SHALL NOT be rendered at full completion (`completed === total`). At full completion the consuming row surfaces its trailing `✓` glyph in place of the meter (see the spec-browser *Change-Row Completion Glyph* and *Tasks Artifact Node Progress* requirements), so the meter only ever depicts genuinely-in-progress work.

The fill width MAY animate via a CSS `transition` so that a watcher-driven completion change visibly nudges the bar. This transition SHALL be disabled under `@media (prefers-reduced-motion: reduce)`.

#### Scenario: In-progress meter is an outlined track with a green fill

- **WHEN** the meter is rendered for a change with at least one incomplete task
- **THEN** it renders an outlined transparent-background track with an inner fill in `--ok`
- **AND** the fill's inline width is proportional to `completed / total`
- **AND** no digits are rendered inside the meter

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
