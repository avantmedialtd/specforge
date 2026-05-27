## REMOVED Requirements

### Requirement: Top-Level Row Display Name and Tint

**Reason**: The full-row background tint reads as overdone in the running app — three coloured bands stacked vertically dominate a sidebar whose other vocabulary is deliberately minimal. The earlier `flatten-sidebar-rows` iteration (commit `c14f234`) addressed the rounded-pill silhouette but not the underlying area-coverage problem. The identity channel still has value, so it moves to a smaller surface rather than being dropped.

**Migration**: Replaced by the new `Top-Level Row Display Name and Swatch` requirement (added in this change) and the new `Inter-Workspace Divider` requirement. Display-name handling is unchanged; only the identity-colour rendering surface and the section-separation signal are reshaped.

## ADDED Requirements

### Requirement: Top-Level Row Display Name and Swatch

The tree pane SHALL render every top-level row — a flat workspace node or a repository group node — using the row's configured display name when one is set, and using the row's derived default name (the folder basename for a flat workspace, the main worktree's basename for a repository group) when none is set. The tree pane SHALL render an 8px filled circular swatch glyph between the row's chevron and its label, in the colour corresponding to the row's configured palette colour, when one is set. When no palette colour is configured the swatch SHALL be omitted.

The swatch SHALL be applied to the top-level row only. Child rows (logical changes, instances, artifact nodes, sections, tasks, capability spec rows) SHALL NOT render a swatch. The row's background SHALL be the default row background regardless of palette colour. The existing selection treatment (a 2px `--accent` `border-left`) SHALL compose with the swatch without modification: the swatch sits in the row's content area, the selection bar lives in the inline-start border slot, and the two signals do not overlap.

#### Scenario: Top-level row uses configured display name

- **WHEN** a flat workspace has a configured display name
- **THEN** its top-level tree row renders with that display name
- **AND** the configured name is also used wherever the row is referenced (for example, the row's accessible label)

#### Scenario: Top-level row falls back to derived name when no display name is configured

- **WHEN** a flat workspace or a repository group has no configured display name
- **THEN** its top-level tree row renders with the folder basename (or main worktree basename, for a repository group)

#### Scenario: Top-level row shows the configured palette colour as a swatch

- **WHEN** a flat workspace or a repository group has a configured palette colour
- **THEN** its top-level tree row renders an 8px filled circular swatch between the chevron and the label, in the colour corresponding to that palette token
- **AND** the row background is the default row background, unchanged by the palette colour
- **AND** child rows below it render no swatch and the default row background

#### Scenario: Top-level row omits the swatch when no palette colour is configured

- **WHEN** a flat workspace or a repository group has no configured palette colour
- **THEN** its top-level tree row renders no swatch
- **AND** the row background is the default row background, indistinguishable from the same row before the presentation store was introduced

#### Scenario: Selection highlight composes with the swatch

- **WHEN** the user selects a top-level row that has a configured palette colour
- **THEN** the row renders both the 2px `--accent` left border bar and the 8px swatch
- **AND** the two signals do not overlap visually (the bar is in the inline-start border slot; the swatch is in the row's content area)
- **AND** the row background is unchanged by the selected state

#### Scenario: Presentation update re-renders the row without a manual refresh

- **WHEN** the user changes the display name or palette colour of a workspace from the Settings view
- **THEN** the corresponding top-level row in the tree pane updates to reflect the new name and swatch without the user having to close and reopen the window or otherwise force a refresh

### Requirement: Inter-Workspace Divider

Successive top-level rows in the tree pane SHALL be separated by a 1px `var(--border)` horizontal hairline. The hairline SHALL be rendered as a `border-top` on every top-level row except the first, so that the first top-level row carries no top border and every subsequent top-level row carries one. The hairline replaces the section-header affordance previously provided by the full-row background tint.

The hairline SHALL apply only to top-level rows (flat workspace nodes and repository group nodes). Child rows SHALL NOT render a `border-top`. The hairline SHALL compose with the row's other visual signals — the swatch in the content area, the selection bar in the inline-start border slot, and any hover/focus state — without modification: it is a cross-axis 1px line and does not occupy the inline-start border slot.

#### Scenario: Second and subsequent workspaces render a hairline

- **WHEN** the tree pane renders two or more top-level rows
- **THEN** the second and every subsequent top-level row resolves a 1px `var(--border)` `border-top`
- **AND** the first top-level row resolves a `border-top` of `0`

#### Scenario: Child rows render no hairline

- **WHEN** a top-level row is expanded
- **THEN** none of its child rows (changes, instances, artifacts, sections, tasks, capability specs) renders a `border-top`
- **AND** the only horizontal separation between successive child rows is the row's vertical padding

#### Scenario: Hairline composes with selection and swatch on the same row

- **WHEN** the user selects a top-level row that is not the first top-level row and that has a configured palette colour
- **THEN** the row simultaneously renders the 1px `var(--border)` `border-top`, the 2px `--accent` `border-left`, and the 8px swatch in the content area
- **AND** no signal visually displaces or hides any other
