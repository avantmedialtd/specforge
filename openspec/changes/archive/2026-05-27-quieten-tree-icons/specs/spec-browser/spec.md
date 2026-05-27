## ADDED Requirements

### Requirement: Artifact Row Presence Treatment

For each artifact node (Proposal, Specs, Design, Tasks) rendered under an instance row or a flat-change row, the row's *leading slot* (the position to the immediate right of the chevron/spacer) SHALL be reserved for identity affordances only — the row SHALL NOT render an icon whose sole semantics are "the underlying artifact file is present on disk." When the artifact's underlying file is present, the row SHALL display only the chevron (or chevron-spacer), the row label, and any trailing meta the schema defines.

When the artifact's underlying file is absent, the row SHALL:

- render at `opacity: 0.45` of the row's normal appearance,
- set `pointer-events: none` (or otherwise be inert to mouse interaction) so that clicking the row produces no selection, no detail-pane change, and no hover styling,
- preserve its layout footprint (chevron-spacer, label, depth indent) so the four-row artifact block does not collapse,
- continue to be visible in the tree as a slot indicator for the missing artifact.

The Specs artifact node SHALL count as "present" iff at least one capability spec file is parsed under the change; otherwise it SHALL be treated as absent and dimmed per the rule above.

#### Scenario: Present artifact rows carry no leading existence icon

- **WHEN** an artifact node is rendered for an artifact whose underlying file is present
- **THEN** the row displays no leading existence-marker glyph (no `Check`, no `DotOutline`, no equivalent)
- **AND** the row renders at full opacity
- **AND** the row participates normally in click, hover, and selection

#### Scenario: Missing artifact rows are dimmed and non-interactive

- **WHEN** an artifact node is rendered for an artifact whose underlying file is absent
- **THEN** the row renders at `opacity: 0.45`
- **AND** the row does not respond to clicks (no selection, no detail-pane change)
- **AND** the row does not display a hover background
- **AND** the row still occupies its full layout slot (label visible, depth indent preserved) so the four-artifact block remains intact

#### Scenario: Specs artifact dimming follows capability-spec presence

- **WHEN** a change has no parsed capability spec files
- **THEN** the Specs artifact row is treated as absent and rendered dim + non-interactive
- **AND** when at least one capability spec file is parsed, the Specs row renders normally

### Requirement: Change-Row Completion Glyph

For change-aggregating rows that surface a task progress count — specifically the flat-workspace change row (`FlatChangeNode`) and the per-instance row (`InstanceNode`) — when every parsed task in the change is complete (`totalTasks > 0` and `completedTasks === totalTasks`), the row SHALL render a trailing `Check` glyph in the row's meta cluster, alongside the progress count. The glyph SHALL appear adjacent to the progress count (between progress and any modification-time element). When at least one task is incomplete, or when the change has no tasks at all, the row SHALL NOT render the trailing `Check` glyph.

The `Check` glyph SHALL NOT appear in the row's leading slot on either row type. Pre-existing leading-position completion markers (specifically the leading `Check` on `FlatChangeNode` rendered when all tasks were done) SHALL be removed.

#### Scenario: Flat-change row gets a trailing tick when all tasks complete

- **WHEN** a flat-workspace change row is rendered for a change with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph alongside the progress count
- **AND** no `Check` glyph appears in the row's leading slot

#### Scenario: Instance row gets a trailing tick when all tasks complete

- **WHEN** a per-instance change row is rendered for an instance with at least one task and every task complete
- **THEN** the row's trailing meta cluster contains a `Check` glyph alongside the progress count
- **AND** the glyph sits adjacent to the progress count, between progress and the modification-time element

#### Scenario: Rows without complete tasks have no trailing tick

- **WHEN** a flat-change row or instance row is rendered for a change with at least one incomplete task, or for a change with no tasks at all
- **THEN** the row's meta cluster contains no `Check` glyph
- **AND** the leading slot also contains no `Check` glyph
