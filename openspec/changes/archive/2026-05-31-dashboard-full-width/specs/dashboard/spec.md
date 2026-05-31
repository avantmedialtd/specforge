## ADDED Requirements

### Requirement: Dashboard Fills Available Width

The Dashboard SHALL fill the full available width of the center (detail) pane at any window size, rather than capping its content at a fixed maximum width or centering it within a narrower column. The Dashboard SHALL retain its surrounding padding. The widths and behaviour of the surrounding shell — the tree (sidebar) pane and the commit-graph rail — SHALL be unaffected; only the Dashboard's own content width follows the pane.

#### Scenario: Wide pane has no dead gutters

- **WHEN** the Dashboard renders in a center pane wider than its former cap
- **THEN** the Dashboard content extends to the full width of the pane (minus its padding)
- **AND** no centered fixed-width column with empty gutters on either side is shown

#### Scenario: Content reflows to fill

- **WHEN** the available pane width increases
- **THEN** the Dashboard's proportional panels and grids reflow to occupy the additional width
- **AND** no horizontal scrollbar is introduced by the Dashboard content

#### Scenario: Surrounding shell is unaffected

- **WHEN** the Dashboard is the active center-pane surface
- **THEN** the sidebar pane and the commit-graph rail retain their existing widths and behaviour
