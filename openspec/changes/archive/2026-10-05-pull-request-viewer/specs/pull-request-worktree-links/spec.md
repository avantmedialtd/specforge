## MODIFIED Requirements

### Requirement: Pull-Request Rows Lead to Their Worktree

In the BitBucket panel and the GitHub panel, a row linked to at least one worktree SHALL show a **worktree marker** beside the row, naming the first linked worktree by its branch (or folder basename when the branch is unknown) and listing every linked worktree in its tooltip and accessible name. The marker SHALL be its own control, separate from the row's control that opens the pull request, and SHALL be reachable by keyboard.

Activating the row SHALL open the pull request in SpecForge, a click showing it in the center pane at its pull-request address (see the *Opening a Pull Request* requirement in the `bitbucket-pull-requests` capability and the *Opening a GitHub Pull Request* requirement in the `github-pull-requests` capability). Because the row opens in SpecForge as well, the marker's tooltip and accessible name SHALL NOT describe the marker as opening in SpecForge; they SHALL name the worktrees it leads to.

Activating the marker SHALL navigate SpecForge, on both transports, to the first linked worktree:

- when that worktree hosts exactly one active change, to that change's default artifact in that worktree's instance;
- when it hosts several, to the default artifact of the one most recently modified, in that instance;
- when it hosts none, to the repository's file browser.

Activating the marker SHALL NOT open the pull request, SHALL NOT navigate the browser skin's page away from SpecForge, and SHALL NOT act on the serving host. A row with no linked worktree SHALL show no marker and SHALL render as it does without this capability.

#### Scenario: The marker opens the change in its worktree

- **WHEN** a linked row's worktree hosts the single active change `add-rate-limits` and the user activates the marker
- **THEN** the detail pane shows `add-rate-limits`'s default artifact read from that worktree
- **AND** the address names that instance when the change has several

#### Scenario: A worktree without a change opens the file browser

- **WHEN** the linked worktree hosts no active change and the user activates the marker
- **THEN** the repository's file browser opens

#### Scenario: The marker and the row do different things

- **WHEN** the user activates the row itself rather than the marker
- **THEN** the center pane shows the pull request at its pull-request address
- **AND** it does not show the worktree's change that the marker leads to

#### Scenario: Several linked worktrees are all named

- **WHEN** a pull request is linked to two worktrees
- **THEN** the marker names the first and its tooltip lists both
- **AND** activating it navigates to the first

#### Scenario: The marker's label leaves opening in SpecForge to the row

- **WHEN** a linked row's worktree marker is rendered
- **THEN** its tooltip and accessible name list the linked worktrees
- **AND** neither of them reads "Open in SpecForge"
