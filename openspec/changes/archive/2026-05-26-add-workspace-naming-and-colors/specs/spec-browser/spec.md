## ADDED Requirements

### Requirement: Top-Level Row Display Name and Tint

The tree pane SHALL render every top-level row — a flat workspace node or a repository group node — using the row's configured display name when one is set, and using the row's derived default name (the folder basename for a flat workspace, the main worktree's basename for a repository group) when none is set. The tree pane SHALL render the row's background with the tint corresponding to the row's configured palette colour when one is set, and with the default row background when none is set.

The tint SHALL be applied to the top-level row only. Child rows (logical changes, instances, artifact nodes, sections, tasks, capability spec rows) SHALL NOT inherit the tint and SHALL continue to render with the default row background. The tint MUST compose cleanly with the existing selection highlight so a selected top-level row remains visually distinct from its unselected neighbours.

When the row's configured palette colour is absent (either because no presentation entry exists, or because the user has explicitly chosen "none"), the row SHALL render with no tint, identical to today's behaviour for that row.

#### Scenario: Top-level row uses configured display name

- **WHEN** a flat workspace has a configured display name
- **THEN** its top-level tree row renders with that display name
- **AND** the configured name is also used wherever the row is referenced (for example, the row's accessible label)

#### Scenario: Top-level row falls back to derived name when no display name is configured

- **WHEN** a flat workspace or a repository group has no configured display name
- **THEN** its top-level tree row renders with the folder basename (or main worktree basename, for a repository group)

#### Scenario: Top-level row is tinted with the configured palette colour

- **WHEN** a flat workspace or a repository group has a configured palette colour
- **THEN** its top-level tree row background renders with the tint corresponding to that colour token
- **AND** child rows below it render with the default row background, not the tint

#### Scenario: Top-level row is untinted when no palette colour is configured

- **WHEN** a flat workspace or a repository group has no configured palette colour
- **THEN** its top-level tree row background renders with the default row background, indistinguishable from the same row before the presentation store was introduced

#### Scenario: Selection highlight remains visible over the tint

- **WHEN** the user selects a tinted top-level row
- **THEN** the row's selected state remains visually distinct from its unselected appearance
- **AND** the configured tint is still discernible underneath the selection treatment

#### Scenario: Presentation update re-renders the row without a manual refresh

- **WHEN** the user changes the display name or palette colour of a workspace from the Settings view
- **THEN** the corresponding top-level row in the tree pane updates to reflect the new name and tint without the user having to close and reopen the window or otherwise force a refresh
