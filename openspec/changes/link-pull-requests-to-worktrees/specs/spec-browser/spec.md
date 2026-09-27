## ADDED Requirements

### Requirement: Pull-Request Chip in the Change Header

When the worktree an active change's artifact is read from is linked to one or more pull requests (see the *Matching a Pull Request to a Worktree* requirement in the `pull-request-worktree-links` capability), the identity row of the change header (see *Change Identity Header in the Detail Pane*) SHALL show a **pull-request chip** for each linked pull request, following the branch chip, up to two; any further linked pull requests SHALL be summarised by one passive `+N` chip whose tooltip lists them.

A pull-request chip SHALL read `#` followed by the pull request's number, and SHALL carry the pull request's draft marker, its checks state and its conflicting marker using the same treatments the pull-request panels use. Its tooltip and accessible name SHALL state the provider, the destination repository, the title, whether it is the viewer's own or awaiting their review, the review summary in words, and the checks state in words. The chip SHALL be an outlined chip in neutral ink, never tinted with the workspace's palette colour, so it is not mistaken for the branch chip.

A pull-request chip is a **control**: activating it by click, Enter or Space SHALL open the pull request exactly as activating its row in a pull-request panel does — through `open_pull_request` on the desktop, and as a link opening a new opener-isolated tab in the browser skin. It SHALL be a separate element from the change name, so the name's copy-on-click, its selection, its single tab stop and its confirmation are unaffected, and it SHALL follow the change name in keyboard order.

An archived change, a change in a flat workspace, and a change whose worktree has no branch SHALL show no pull-request chip. A change whose worktree is linked to nothing SHALL render its header exactly as without this requirement.

#### Scenario: A linked worktree shows its pull request

- **WHEN** the detail pane renders an artifact read from a worktree linked to GitHub pull request 42, which has failing checks
- **THEN** the identity row shows the branch chip followed by a chip reading `#42` with the failing checks treatment
- **AND** the chip's accessible name states GitHub, the repository, the title and that the checks are failing

#### Scenario: The chip opens the pull request

- **WHEN** the user activates the pull-request chip in the desktop application
- **THEN** the pull request's web page opens in the system browser and the window does not navigate

#### Scenario: Copying the name ignores the chip

- **WHEN** the user clicks the change name in a header that shows a pull-request chip
- **THEN** the clipboard contains the change name only

#### Scenario: More than two linked pull requests are summarised

- **WHEN** the worktree is linked to three pull requests
- **THEN** the identity row shows two pull-request chips and a `+1` chip whose tooltip names the third

#### Scenario: An archived change shows no pull-request chip

- **WHEN** the detail pane renders an artifact of an archived change whose worktree path is linked to a pull request
- **THEN** no pull-request chip is rendered

### Requirement: Pull-Request Marker in the Instance Switcher

In the instance switcher (see *Instance Switcher in the Change Header*), the control for an instance whose worktree is linked to one or more pull requests SHALL show, after its branch chip and before its divergence label, a passive marker reading `#` followed by the first linked pull request's number, and `+N` when more are linked. The marker SHALL NOT be separately focusable or activatable — the control remains one control whose activation switches instance — and the control's accessible name SHALL include the linked pull requests' numbers. An instance whose worktree is linked to nothing SHALL render its control exactly as without this requirement, and the read-only form SHALL show no marker.

#### Scenario: The switcher shows which instance has a pull request

- **WHEN** a change is hosted in two worktrees and only the feature worktree is linked to pull request 7
- **THEN** the feature worktree's control shows `#7` after its branch chip
- **AND** the other control shows no marker

#### Scenario: The marker does not change what the control does

- **WHEN** the user activates a control that shows a pull-request marker
- **THEN** the detail pane switches to that instance
- **AND** no pull request is opened
