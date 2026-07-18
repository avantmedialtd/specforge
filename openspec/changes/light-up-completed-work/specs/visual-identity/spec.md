## MODIFIED Requirements

### Requirement: Outlined Chip Badges

Status badges (e.g., `DIVERGED`, branch-name labels, change-id labels, count badges) SHALL render as outlined chips with `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase` (where applicable), `letter-spacing: 0.05em`, `font-family: var(--font-mono)`, and `font-size: var(--text-xs)`. The previous tinted-fill pill style MUST NOT be used.

Where horizontal space is tight, a status indicator MAY collapse to a 4px circular dot rendered in the same color (`--warn` for problem states, `--ok` for healthy states). A dot indicator SHALL always carry a `title` attribute with the full label for hover disclosure and accessibility.

Informational chips and status dots SHALL remain outlined / transparent and SHALL NOT carry any `box-shadow` glow or halo. The accent and status glow tokens (`--accent-glow`, `--shadow-accent`, `--shadow-accent-strong`, `--glow-ok`) are reserved EXCLUSIVELY for the selected tree row, the primary button, focused inputs, the focus ring, and the in-progress meter.

The row grammar sanctions exactly **two** filled elements, and they are symmetric — the two task-states of a change. The **task-progress meter** (see the *Task Progress Meter* requirement) is the *in-progress* fill: a fill rendered in `--ok` inside an outlined track. The **completion mark** (see the *Completed-State Styling* requirement) is the *done* fill: a solid `--ok-strong` disc with a knocked-out check. Neither is a chip fill; the selected-row accent wash and the primary-button fill are governed by the *Tree Row Selection Model* and *Accent Color* requirements respectively. No OTHER chip, badge, or status dot SHALL use a filled background. Of the two sanctioned filled elements only the in-progress meter MAY carry a glow (its optional `--glow-ok` halo); the completion mark SHALL carry none, so the reserved-glow set above is unchanged.

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

## ADDED Requirements

### Requirement: Completed-State Styling

The workspace tree SHALL render completion in the success-green family rather than in a muted or dimmed neutral, so that a finished item reads as complete at a glance. This applies at three levels — the milestone completion glyph, a completed change's rail, and a completed leaf task — and at every level a colour-independent shape carries the "done" meaning while green provides reinforcement (so the signal survives colour-vision deficiency).

A dedicated foreground token `--ok-strong` SHALL provide the "done" green for these marks, distinct from `--ok` (which is tuned as the fill inside the outlined task-progress meter). `--ok` used as a foreground on the light scheme's white `--surface` is only ~2.6:1 — below AA for text and below the 3:1 non-text floor — so `--ok-strong` SHALL be a deeper green: `#047857` on light (≥4.5:1 on `--surface`) and `#34d399` on dark (9.34:1 on `--surface`). `--ok` retains its single existing role as the progress-meter fill.

**Milestone completion glyph.** When a Section, the Tasks artifact node, or a whole change/instance is fully complete, the trailing completion glyph SHALL render as a solid `--ok-strong` disc with a knocked-out check (a check in `--surface`, punched through to the surface plane), NOT as a muted outline checkmark. This disc is the *done* fill sanctioned by the *Outlined Chip Badges* requirement. It SHALL carry no `box-shadow` glow or halo. It SHALL be visually distinct from a 4px status dot — larger (on the order of 15px) and carrying an interior check — so it is not mistaken for a status dot.

**Completed-change rail.** A completed two-line change row SHALL render its left rail in `--ok-strong`, replacing the workspace-palette-colour rail (`tree-row--rail-{color}`) it would otherwise show. Selection SHALL still win: when a completed change row is selected, the `--accent` selection bar defined by the *Tree Row Selection Model* requirement SHALL override the completion rail, exactly as it overrides the workspace-colour rail. A completed change SHALL NOT receive a full-row background wash; the full-row wash remains the exclusive signal of selection.

**Completed leaf task.** A completed leaf task row SHALL render its label in `--ok-strong` and SHALL retain its `text-decoration: line-through`. The line-through is the colour-independent "done" signal for a leaf task (which carries no glyph); the green is reinforcement. A completed leaf task SHALL NOT receive the filled completion disc — the disc is reserved for milestone completion (Section / change), keeping the atom lighter than the milestone.

The foreground green `--ok-strong` covers the disc fill, the rail, and the completed-task label; the disc's knocked-out check resolves from `--surface`. `--ok-strong` clears AA for the completed-task text in both schemes.

#### Scenario: Milestone completion glyph is a filled ok disc

- **WHEN** a Section, the Tasks artifact node, or a whole change is fully complete and its trailing completion glyph is rendered
- **THEN** the glyph is a solid `--ok-strong` disc with a knocked-out `--surface` check, not a muted outline checkmark
- **AND** the disc carries no glow
- **AND** the disc is larger than, and distinguishable from, a 4px status dot

#### Scenario: Completed change shows an ok rail

- **WHEN** an unselected two-line change row is fully complete
- **THEN** its left rail renders in `--ok-strong` rather than in the workspace palette colour
- **AND** the row background is not washed (no full-row completion tint)

#### Scenario: Selection overrides the completed-change rail

- **WHEN** a fully-complete change row is selected
- **THEN** the `--accent` selection bar and wash are shown, overriding the `--ok-strong` completion rail
- **AND** when the row is deselected the `--ok-strong` completion rail returns

#### Scenario: Completed leaf task is green and struck

- **WHEN** a leaf task is complete
- **THEN** its label renders in `--ok-strong` with `text-decoration: line-through`
- **AND** it does not render the filled completion disc

#### Scenario: Done state reads without colour

- **WHEN** any completed item is rendered
- **THEN** a colour-independent shape signals completion — the check inside the disc for a milestone, the line-through for a leaf task
- **AND** the green (`--ok-strong`) reinforces but is not the sole carrier of the completion meaning
