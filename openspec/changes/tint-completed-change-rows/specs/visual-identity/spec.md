## MODIFIED Requirements

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
