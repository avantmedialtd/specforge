## MODIFIED Requirements

### Requirement: Reactive Updates from Filesystem

The tree pane and the detail pane SHALL reflect on-disk changes within the watcher's debounce window without requiring user action. After the watcher finishes processing a debounced batch of filesystem events, the *first* refresh the frontend performs in response to that batch SHALL observe the post-batch state — the UI MUST NOT lag behind by one event for any on-disk change, including content-only changes inside a change directory that is already tracked (artifact file creation, task checkbox toggles, edits to spec or proposal markdown).

#### Scenario: Tree updates when new change appears

- **WHEN** a new change directory is created on disk in a registered workspace
- **THEN** the new change appears as a child of that workspace in the tree

#### Scenario: Detail pane updates when shown file is edited

- **WHEN** the detail pane is currently rendering an artifact's markdown
- **AND** that markdown file is modified on disk
- **THEN** the detail pane re-renders with the updated content

#### Scenario: Tree updates when change is archived on disk

- **WHEN** a change directory is moved from `openspec/changes/<id>/` to `openspec/changes/archive/<id>/`
- **THEN** the change is removed from the tree

#### Scenario: Artifact row flips to present when its file is created inside an existing change

- **WHEN** a change directory already exists and is tracked by the watcher (for example because `openspec new change` previously wrote only its `.openspec.yaml`)
- **AND** a subsequent on-disk write creates one of the four artifact files (`proposal.md`, `design.md`, `tasks.md`, or a `specs/<capability>/spec.md`) inside that change directory
- **THEN** the corresponding artifact row in the tree re-renders as present (full opacity, interactive) within the watcher's debounce window
- **AND** the row reaches its present state on the first refresh the frontend performs after that write — no further on-disk edit or user action is required to flip the row

#### Scenario: Instance-row task progress updates when a checkbox is toggled

- **WHEN** an instance row (or, for a singleton logical change, the flattened row) is rendered in the tree with a task-progress count
- **AND** an on-disk edit to that change's `tasks.md` flips a task line's checkbox between `- [ ]` and `- [x]`
- **THEN** the row's `(completed/total)` progress label re-renders with the new completion count within the watcher's debounce window
- **AND** the new count is visible on the first refresh the frontend performs after that edit — no further edit, focus change, or window action is required to surface it

#### Scenario: Section completion glyph and auto-collapse update when the last task in a section is toggled

- **WHEN** a Section node is rendered expanded with at least one incomplete task
- **AND** an on-disk edit to `tasks.md` toggles the last incomplete task in that section from `- [ ]` to `- [x]`
- **THEN** the Section row's trailing `✓` glyph and the Section's auto-collapsed rendering both appear within the watcher's debounce window
- **AND** both are visible on the first refresh the frontend performs after that edit
