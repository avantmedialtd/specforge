## MODIFIED Requirements

### Requirement: Live Updates From the Watcher

The interactive frontend SHALL subscribe to the application service's filesystem-change broadcast and refresh affected views when changes occur, without the user re-issuing a command. On a change event the frontend SHALL re-read the current aggregated view from the service rather than maintaining an independent cache.

The refresh SHALL include the body of the artifact currently shown in the detail pane, re-read from the service, even when the change event leaves the user's selection unchanged. This gives the terminal frontend the same detail-pane freshness the desktop shell provides — see the *Reactive Updates from Filesystem* requirement in the `spec-browser` capability.

A re-read the user did not initiate SHALL preserve the detail pane's scroll offset, clamped so that at least one line of the new body remains visible, and SHALL leave the displayed body in place when the read fails, rather than replacing it with the failure message. A load the user initiated — selecting a different change, or switching artifact tab — SHALL continue to reset the offset to the top and to render a read failure in place of the body. A re-read whose reply arrives after the user has moved to a different selection or tab SHALL be discarded rather than displayed; when such a re-read supersedes a still-outstanding user-initiated load, it SHALL adopt that load's presentation so the reader still arrives at the top of the artifact they chose.

#### Scenario: A new change appears

- **WHEN** a new change directory appears in a watched workspace while the interactive frontend is open
- **THEN** the tree updates to include it without user action

#### Scenario: The open artifact's body refreshes without a selection change

- **WHEN** the detail pane is showing an artifact and the user's selection has not moved
- **AND** that artifact's file is modified on disk
- **THEN** the detail pane shows the updated content without user action

#### Scenario: A shrunken body clamps the preserved offset

- **WHEN** the detail pane is showing an artifact and the user has scrolled deep into it
- **AND** an on-disk edit shortens that artifact to fewer lines than the current offset
- **THEN** the detail pane shows the new body with its offset reduced so content is still visible
- **AND** the pane is not left blank

#### Scenario: An artifact that appears becomes reachable without moving the cursor

- **WHEN** the detail pane is showing a change whose only artifact is its proposal
- **AND** an on-disk write adds `design.md` or `tasks.md` to that change
- **THEN** the artifact tab strip offers the new artifacts without the user moving the tree cursor off the change and back
- **AND** the tab the user was reading remains the active tab

#### Scenario: Scroll offset survives a watcher-driven re-read

- **WHEN** the detail pane is showing an artifact and the user has scrolled down within it
- **AND** a filesystem change triggers a re-read of that artifact
- **THEN** the detail pane's scroll offset is unchanged

#### Scenario: A failed re-read leaves the reader's content in place

- **WHEN** the detail pane is showing an artifact
- **AND** a re-read the user did not initiate fails, because the artifact was archived or removed or is mid-write
- **THEN** the detail pane continues to show the body it already had, at its existing scroll offset
- **AND** the failure message does not replace it

#### Scenario: A re-read that supersedes the user's own load still starts at the top

- **WHEN** the user selects a change and a filesystem change triggers a re-read before that selection's body has arrived
- **THEN** the detail pane shows the selected artifact with its scroll offset at the top

#### Scenario: Selecting a different change still starts at the top

- **WHEN** the user selects a different change, or switches to a different artifact tab
- **THEN** the detail pane loads that artifact and its scroll offset returns to the top

#### Scenario: Long-running computation does not block input

- **WHEN** a dashboard refresh that performs git scans is in progress
- **THEN** the interface continues to accept and respond to keyboard input
