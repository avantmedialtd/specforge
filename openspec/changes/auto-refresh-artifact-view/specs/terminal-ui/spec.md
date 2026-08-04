## MODIFIED Requirements

### Requirement: Live Updates From the Watcher

The interactive frontend SHALL subscribe to the application service's filesystem-change broadcast and refresh affected views when changes occur, without the user re-issuing a command. On a change event the frontend SHALL re-read the current aggregated view from the service rather than maintaining an independent cache.

The refresh SHALL include the body of the artifact currently shown in the detail pane, re-read from the service, even when the change event leaves the user's selection unchanged. This gives the terminal frontend the same detail-pane freshness the desktop shell provides — see the *Reactive Updates from Filesystem* requirement in the `spec-browser` capability.

A re-read the user did not initiate SHALL preserve the detail pane's scroll offset. A load the user initiated — selecting a different change, or switching artifact tab — SHALL continue to reset the offset to the top. A re-read whose reply arrives after the user has moved to a different selection or tab SHALL be discarded rather than displayed.

#### Scenario: A new change appears

- **WHEN** a new change directory appears in a watched workspace while the interactive frontend is open
- **THEN** the tree updates to include it without user action

#### Scenario: The open artifact's body refreshes without a selection change

- **WHEN** the detail pane is showing an artifact and the user's selection has not moved
- **AND** that artifact's file is modified on disk
- **THEN** the detail pane shows the updated content without user action

#### Scenario: Scroll offset survives a watcher-driven re-read

- **WHEN** the detail pane is showing an artifact and the user has scrolled down within it
- **AND** a filesystem change triggers a re-read of that artifact
- **THEN** the detail pane's scroll offset is unchanged

#### Scenario: Selecting a different change still starts at the top

- **WHEN** the user selects a different change, or switches to a different artifact tab
- **THEN** the detail pane loads that artifact and its scroll offset returns to the top

#### Scenario: Long-running computation does not block input

- **WHEN** a dashboard refresh that performs git scans is in progress
- **THEN** the interface continues to accept and respond to keyboard input
