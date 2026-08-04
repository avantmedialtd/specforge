## MODIFIED Requirements

### Requirement: Dashboard Home Surface

The application SHALL provide a Dashboard: a global, read-only overview rendered in the center (detail) pane. The Dashboard SHALL be the center pane's default render target — it SHALL be shown whenever the current address does not name another view, and whenever no artifact and no commit is selected, in place of any "nothing selected" placeholder.

At startup the Dashboard SHALL be shown when no address is supplied or the supplied address names the home surface. When an explicit address names another view, that view SHALL be rendered instead — see the *Cold-Load Address Resolution* requirement in the `view-routing` capability. An address that cannot be resolved SHALL be reported as not found rather than silently falling back to the Dashboard.

The tree pane SHALL render a pinned "Dashboard" entry at the top of the pane (mirroring the pinned Settings entry at the bottom). Selecting the Dashboard entry SHALL set the center pane to the Dashboard. Selecting a renderable artifact in the tree, or a commit in the rail, SHALL replace the Dashboard with that target; selecting the Dashboard entry again SHALL return the center pane to the Dashboard. The Dashboard entry SHALL convey an active treatment while the Dashboard is the current center-pane target.

#### Scenario: Dashboard shown at startup

- **WHEN** the user opens the main window with no address supplied and no artifact or commit has been selected
- **THEN** the center pane renders the Dashboard
- **AND** no "nothing selected" placeholder is shown

#### Scenario: An explicit address opens its view instead of the Dashboard

- **WHEN** the application is opened at an address naming a change artifact in a registered workspace
- **THEN** the center pane renders that artifact
- **AND** the Dashboard is not the center pane's target

#### Scenario: An unresolvable address does not silently fall back

- **WHEN** the application is opened at an address that cannot be resolved
- **THEN** the user is told the address could not be found
- **AND** the Dashboard is not rendered as though the address had named it

#### Scenario: Dashboard entry returns to the Dashboard

- **WHEN** the center pane is rendering an artifact or a commit detail
- **AND** the user selects the pinned Dashboard entry at the top of the tree
- **THEN** the center pane renders the Dashboard
- **AND** the Dashboard entry renders in its active state

#### Scenario: Selecting an artifact replaces the Dashboard

- **WHEN** the center pane is rendering the Dashboard
- **AND** the user selects a renderable artifact node in the tree
- **THEN** the center pane renders that artifact's markdown
- **AND** the Dashboard entry returns to its idle state
