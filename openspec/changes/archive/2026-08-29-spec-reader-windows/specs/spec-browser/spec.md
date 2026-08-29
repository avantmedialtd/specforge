# spec-browser Delta — Reader Windows for Workspace Documents

## MODIFIED Requirements

### Requirement: Reactive Updates from Filesystem

The tree pane and every **document surface** SHALL reflect on-disk changes within the watcher's debounce window without requiring user action. A document surface is any surface that renders one markdown document: the detail pane, the workspace file browser's preview region, and a reader window (see the `reader-window` capability). Every guarantee this requirement makes about a document surface binds all three; where the text below says "the pane", it is naming the behaviour of a document surface, not the detail pane alone.

After the watcher finishes processing a debounced batch of filesystem events, the *first* refresh the frontend performs in response to that batch SHALL observe the post-batch state — the UI MUST NOT lag behind by one event for any on-disk change, including content-only changes inside a change directory that is already tracked (artifact file creation, task checkbox toggles, edits to spec or proposal markdown).

A document surface's refresh SHALL re-read the document it is currently rendering. It SHALL be driven by the change notification alone and MUST NOT be conditioned on the workspace named in that notification's payload, because a notification MAY carry any tracked workspace as a carrier rather than the workspace whose contents changed.

A document surface's freshness SHALL NOT depend on where in the workspace its document lies. For a document beneath a change directory the workspace watcher supplies the notification; for any other markdown document — a capability specification beneath `openspec/specs/`, or a file anywhere else in the browse root — a document watch supplies it, per the `document-watch` capability. Both deliver the same guarantee to the surface, and a surface SHALL NOT be required to know which mechanism notified it.

A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT alter the rendered document when the document's bytes are unchanged. (This narrows a previous contract, under which such a refresh was required to be wholly unobservable when the bytes were unchanged. That is no longer correct: a file rewritten with identical bytes has a **new modification time**, and the header reports modification time — see *Change Identity Header in the Detail Pane*, "Last changed". The protection this clause exists to give — an undisturbed reader, no loading indicator, no repaint of the document — is unchanged; what narrows is its scope, from the whole pane to the document within it.) When such a refresh fails to read the document, the surface SHALL continue to display the content it already holds rather than replacing it with an error. A read the user initiated by selecting a document retains its existing loading and error presentation.

The cost of the unfiltered subscription SHALL remain bounded by this guarantee: a refresh that changes neither the document's bytes nor its modification time SHALL do no work beyond the read itself, and one that changes only the modification time SHALL NOT re-render the document. Where two mechanisms both notify a surface about one document — possible for a document beneath a change directory, per the *Independent of the Workspace Watcher* requirement in the `document-watch` capability — the surface SHALL coalesce the resulting refreshes, and the duplicate SHALL remain unobservable under the clause above.

#### Scenario: Tree updates when new change appears

- **WHEN** a new change directory is created on disk in a registered workspace
- **THEN** the new change appears as a child of that workspace in the tree

#### Scenario: Detail pane updates when shown file is edited

- **WHEN** the detail pane is currently rendering an artifact's markdown
- **AND** that markdown file is modified on disk
- **THEN** the detail pane re-renders with the updated content

#### Scenario: Every document surface updates when its file is edited

- **WHEN** a document is rendered in the file browser's preview region or in a reader window
- **AND** that file is modified on disk
- **THEN** that surface re-renders with the updated content

#### Scenario: Freshness does not depend on where the document lives

- **WHEN** a document surface is rendering a markdown file that lies outside `openspec/changes/`
- **AND** that file is modified on disk
- **THEN** the surface re-renders with the updated content, exactly as it would for a change artifact

#### Scenario: Reading position survives a refresh the user did not initiate

- **WHEN** a document surface is rendering a document and the user has scrolled away from the top
- **AND** that document's file is modified on disk while the user's selection is unchanged
- **THEN** the surface renders the updated content
- **AND** the reading position is preserved — the surface neither scrolls to the top nor scrolls back to a section or task the user selected in the tree earlier
- **AND** no loading indicator is presented

#### Scenario: Refresh with unchanged content is not observable

- **WHEN** a document surface is rendering a document
- **AND** a filesystem change elsewhere triggers a refresh whose read returns content identical to what is displayed, with an unchanged modification time
- **THEN** the rendered output, the reading position, and the loading indicator are all unchanged

#### Scenario: A duplicate notification for one document is not observable

- **WHEN** a document surface is rendering a document beneath a change directory, for which both the workspace watcher and a document watch deliver a notification for one edit
- **THEN** the resulting refreshes are coalesced
- **AND** the rendered output, the reading position, and the loading indicator show no additional disturbance

#### Scenario: A modification-time-only change updates the header and nothing else

- **WHEN** a document surface is rendering a document
- **AND** a refresh returns content identical to what is displayed but a newer modification time
- **THEN** the header's last-changed label updates
- **AND** the rendered document is not re-rendered
- **AND** the reading position is preserved and no loading indicator is presented

#### Scenario: Refresh is not conditioned on the workspace the notification names

- **WHEN** a document surface is rendering a document belonging to one tracked workspace
- **AND** a filesystem-change notification arrives naming a different tracked workspace
- **THEN** the surface still re-reads its document and renders the current on-disk content

#### Scenario: Failed background read preserves the displayed content

- **WHEN** a document surface is rendering a document
- **AND** a refresh the user did not initiate fails to read that document, because its file was removed, became unreadable, or was caught mid-write
- **THEN** the surface continues to display the content it already loaded
- **AND** no error state replaces it

#### Scenario: Failed selection read still reports the error

- **WHEN** the user selects a document whose file cannot be read
- **THEN** the document surface presents its error state

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
