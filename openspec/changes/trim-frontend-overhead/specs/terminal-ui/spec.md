## ADDED Requirements

### Requirement: Watcher Bursts Coalesce Into One Refresh

The interactive frontend SHALL satisfy every change event that is available when it next handles watcher input with a single refresh. The watcher emits several events for one debounced batch. This requirement refines how the *Live Updates From the Watcher* requirement is met: the frontend still re-reads the current aggregated view from the service and keeps no independent cache, but it does so once per burst instead of once per event.

That single refresh SHALL cover everything a per-event refresh covers: the Browse tree, the Settings workspace mirror, the body of the artifact shown in the detail pane, and the commit graph while the History screen is open. It SHALL keep the presentation rules of *Live Updates From the Watcher*: the preserved and clamped scroll offset, the body kept in place on a failed re-read, replies discarded after the selection or tab moved, and a re-read that supersedes a user-initiated load adopting that load's presentation.

Coalescing SHALL add no delay. The refresh SHALL happen in the same turn of the event loop in which the burst's events are received, not after a timer. When the frontend has fallen behind the broadcast and events were skipped, it SHALL treat that as a change that needs a refresh rather than ignore it. Events that the frontend deliberately does not render, the pull-request snapshot updates, SHALL NOT trigger a refresh.

#### Scenario: A burst of events produces one refresh

- **WHEN** a single watcher batch delivers several change events before the frontend handles any of them
- **THEN** the frontend re-reads the tree, the Settings mirror and the open artifact's body once
- **AND** it issues at most one artifact read for the burst

#### Scenario: The coalesced refresh is not delayed

- **WHEN** a change event arrives while the frontend is idle
- **THEN** the tree reflects it in the same loop turn, without waiting on a timer

#### Scenario: Skipped events still refresh the view

- **WHEN** the frontend falls behind the broadcast and some change events are dropped before it reads them
- **THEN** the frontend still performs a refresh
- **AND** the tree reflects the service's current state

#### Scenario: A pull-request snapshot update does not refresh the tree

- **WHEN** the only events received are pull-request snapshot updates
- **THEN** the frontend performs no refresh and no artifact read

### Requirement: Commit Graph Reloads Are Single-Flight and Current

The History screen SHALL display only a commit graph that answers the frontend's latest graph request. Every graph request SHALL carry a generation. A reply whose generation is not the latest SHALL be discarded. Examples are a reply for a repository the user has since switched away from, or a reply for a smaller history window the user has since extended.

A reload the user initiates, by opening History for a repository or by loading more history, SHALL be issued immediately and SHALL supersede any reload in flight. A reload the watcher triggers SHALL be single-flight: while a graph read is in flight, further watcher-triggered reloads SHALL NOT start another read. They SHALL instead cause exactly one more read once the in-flight read replies, so that the displayed graph ends up reflecting the last change.

#### Scenario: A late reply for the previous repository is discarded

- **WHEN** the user opens History for one repository and then switches to another before the first graph read has replied
- **AND** the first read replies after the second
- **THEN** the History screen shows the second repository's graph

#### Scenario: Loading more history is not undone by an older reply

- **WHEN** the user loads more history while a smaller-window read is still in flight
- **THEN** the History screen ends up showing the larger window

#### Scenario: A watcher burst runs at most one extra graph read

- **WHEN** several watcher refreshes occur while one graph read for the open repository is in flight
- **THEN** no further graph read starts until that read replies
- **AND** exactly one more read is issued after it, and its result is displayed

### Requirement: Rendering Work Is Reused Across Frames

The interactive frontend SHALL NOT repeat rendering work whose inputs have not changed between frames. The styled lines of the detail pane's markdown SHALL be produced once for a given artifact body and active colour scheme, and reused until either changes. The commit rail's lines SHALL be produced once for a given loaded graph and active colour scheme. The selected-row highlight SHALL be applied at draw time, and only the rows in view SHALL be handed to the terminal.

Reuse SHALL be invisible. Every frame SHALL render exactly what rendering from scratch would render. This includes the OSC 8 or textual link presentation chosen under the *Artifact Markdown Rendering* requirement. A scheme change SHALL take effect on the next frame, as the *Colour Scheme Selection* requirement requires. Building the rail for a loaded graph SHALL take time linear in its commits plus its edges.

The frontend SHALL redraw only when something it displays may have changed: after it applies an input, an event or an asynchronous reply, after a terminal resize, and on a periodic tick that refreshes time-dependent surfaces such as the quota countdown. That tick SHALL fire no more often than once per second.

#### Scenario: An unchanged artifact is not re-rendered on the next frame

- **WHEN** the detail pane shows an artifact and a frame is drawn with neither the body nor the scheme changed
- **THEN** the frontend reuses the previously rendered lines rather than parsing the markdown again

#### Scenario: A new body is rendered on the next frame

- **WHEN** the artifact body shown in the detail pane is replaced
- **THEN** the next frame renders the new body, identical to rendering it from scratch

#### Scenario: A scheme change redraws cached content in the new scheme

- **WHEN** the user selects a different colour scheme while an artifact or a commit graph is shown
- **THEN** the next frame shows that content in the new scheme's colours

#### Scenario: Moving the selection in History does not rebuild the rail

- **WHEN** the user moves the selected commit in the History screen without loading a new graph
- **THEN** the highlighted row follows the selection
- **AND** the rail's lines are not rebuilt

#### Scenario: An idle frontend does not redraw continuously

- **WHEN** no input, event or reply arrives
- **THEN** the frontend redraws no more often than once per second
