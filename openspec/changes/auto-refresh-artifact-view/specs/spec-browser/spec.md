## MODIFIED Requirements

### Requirement: Reactive Updates from Filesystem

The tree pane and the detail pane SHALL reflect on-disk changes within the watcher's debounce window without requiring user action. After the watcher finishes processing a debounced batch of filesystem events, the *first* refresh the frontend performs in response to that batch SHALL observe the post-batch state — the UI MUST NOT lag behind by one event for any on-disk change, including content-only changes inside a change directory that is already tracked (artifact file creation, task checkbox toggles, edits to spec or proposal markdown).

The detail pane's refresh SHALL re-read the artifact it is currently rendering. It SHALL be driven by the change notification alone and MUST NOT be conditioned on the workspace named in that notification's payload, because a notification MAY carry any tracked workspace as a carrier rather than the workspace whose contents changed.

A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT be observable at all when the artifact's bytes are unchanged. When such a refresh fails to read the artifact, the pane SHALL continue to display the content it already holds rather than replacing it with an error. A read the user initiated by selecting an artifact retains its existing loading and error presentation.

#### Scenario: Tree updates when new change appears

- **WHEN** a new change directory is created on disk in a registered workspace
- **THEN** the new change appears as a child of that workspace in the tree

#### Scenario: Detail pane updates when shown file is edited

- **WHEN** the detail pane is currently rendering an artifact's markdown
- **AND** that markdown file is modified on disk
- **THEN** the detail pane re-renders with the updated content

#### Scenario: Reading position survives a refresh the user did not initiate

- **WHEN** the detail pane is rendering an artifact and the user has scrolled away from the top
- **AND** that artifact's file is modified on disk while the user's selection is unchanged
- **THEN** the pane renders the updated content
- **AND** the reading position is preserved — the pane neither scrolls to the top nor scrolls back to a section or task the user selected in the tree earlier
- **AND** no loading indicator is presented

#### Scenario: Selecting a section or task still scrolls to it

- **WHEN** the user selects a Section or Task node in the tree
- **THEN** the detail pane scrolls to the corresponding position in the rendered markdown
- **AND** selecting the same node again scrolls to it again

#### Scenario: Refresh with unchanged content is not observable

- **WHEN** the detail pane is rendering an artifact
- **AND** a filesystem change elsewhere triggers a refresh whose read returns content identical to what is displayed
- **THEN** the rendered output, the reading position, and the loading indicator are all unchanged

#### Scenario: Refresh is not conditioned on the workspace the notification names

- **WHEN** the detail pane is rendering an artifact belonging to one tracked workspace
- **AND** a filesystem-change notification arrives naming a different tracked workspace
- **THEN** the pane still re-reads its artifact and renders the current on-disk content

#### Scenario: Failed background read preserves the displayed content

- **WHEN** the detail pane is rendering an artifact
- **AND** a refresh the user did not initiate fails to read that artifact, because its change was archived or its file was removed or is mid-write
- **THEN** the pane continues to display the content it already loaded
- **AND** no error state replaces it

#### Scenario: Failed selection read still reports the error

- **WHEN** the user selects an artifact whose file cannot be read
- **THEN** the detail pane presents its error state

#### Scenario: Tree updates when change is archived on disk

- **WHEN** a change directory is moved from `openspec/changes/<id>/` to `openspec/changes/archive/<id>/`
- **THEN** the change is removed from the tree

#### Scenario: Artifact row flips to present when its file is created inside an existing change

- **WHEN** a change directory already exists and is tracked by the watcher (for example because `openspec new change` previously wrote only its `.openspec.yaml`)
- **AND** a subsequent on-disk write creates one of the four artifact files (`proposal.md`, `design.md`, `tasks.md`, or a `specs/<capability>/spec.md`) inside that change directory
- **THEN** the corresponding artifact row in the tree re-renders as present (full opacity, interactive) within the watcher's debounce window
- **AND** the row reaches its present state on the first refresh the frontend performs after that write — no further on-disk edit or user action is required to flip the row

#### Scenario: Instance-row task progress updates when a checkbox is toggled

- **WHEN** an instance row (or, for a singleton logical change, the flattened row) is rendered in the tree with a task-progress meter
- **AND** an on-disk edit to that change's `tasks.md` flips a task line's checkbox between `- [ ]` and `- [x]`
- **THEN** the row's task-progress meter re-renders with its fill width reflecting the new completion ratio within the watcher's debounce window
- **AND** the new fill is visible on the first refresh the frontend performs after that edit — no further edit, focus change, or window action is required to surface it

#### Scenario: Section completion glyph and auto-collapse update when the last task in a section is toggled

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an on-disk edit to `tasks.md` toggles the last incomplete task in that section from `- [ ]` to `- [x]`
- **THEN** the Section row's trailing `✓` glyph and the Section's auto-collapsed rendering both appear within the watcher's debounce window
- **AND** both are visible on the first refresh the frontend performs after that edit
