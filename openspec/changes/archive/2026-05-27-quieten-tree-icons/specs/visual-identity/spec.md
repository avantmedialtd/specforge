## MODIFIED Requirements

### Requirement: Outlined Chip Badges

Status badges (e.g., `DIVERGED`, branch-name labels, change-id labels) SHALL render as outlined chips with `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase`, `letter-spacing: 0.05em`, `font-family: var(--font-mono)`, and `font-size: var(--text-xs)`. The previous tinted-fill pill style for `row-divergence-*` MUST NOT be used.

Where horizontal space is tight, a status indicator MAY collapse to a 4px circular dot rendered in the same color (`--warn` for problem states, `--ok` for healthy states). A dot indicator SHALL always carry a `title` attribute with the full label for hover disclosure and accessibility.

Missing-artifact rows are NOT represented as outlined chips; they follow the dim-row treatment defined in the spec-browser capability instead.

#### Scenario: Divergence dot replaces a chip in dense rows

- **WHEN** divergence is indicated in a dense row where a full chip would not fit
- **THEN** a 4px circular dot in `--warn` (for diverged) or another status color is rendered instead
- **AND** the dot's container exposes a `title` attribute carrying the human-readable status label

## ADDED Requirements

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
