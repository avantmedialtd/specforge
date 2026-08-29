# workspace-file-browser Delta — Reader Windows for Workspace Documents

## MODIFIED Requirements

### Requirement: Pull-Based Freshness

The **listing** SHALL be fetched when the browser opens and when its browse root changes, and the browser SHALL provide a manual refresh control that re-runs the enumeration. The application SHALL NOT register any filesystem watcher for the listing: enumeration walks or queries the whole browse root, and keeping it continuously fresh would mean watching a workspace-sized tree — the cost this requirement exists to refuse.

The **preview** is not bound by that refusal, because it concerns exactly one file. While a file is selected, the preview SHALL keep that file's rendered content fresh through a document watch (see the `document-watch` capability), registered when the selection is made and released when the selection changes or the browser closes. A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT alter the rendered document when the file's bytes are unchanged, per the *Reactive Updates from Filesystem* requirement in the `spec-browser` capability.

The distinction is between *which files exist*, which stays pull-based, and *what one open file says*, which does not. A file created while the browser is open therefore still requires a refresh to appear in the tree; a file already selected updates in place.

(This narrows a previous contract, under which the application registered no filesystem watcher for the browser at all. The protection that clause exists to give — no watcher scaled to the size of a workspace — is unchanged; what narrows is its scope, from the whole browser to its listing.)

#### Scenario: New file appears after refresh

- **WHEN** a `.md` file is created in the workspace while the browser is open
- **AND** the user activates the refresh control
- **THEN** the new file appears in the tree

#### Scenario: No watcher is registered for the listing

- **WHEN** the file browser is opened for a workspace and no file is selected
- **THEN** no filesystem watcher is created beyond the existing `openspec/`-scoped watcher

#### Scenario: A new file does not appear without a refresh

- **WHEN** a `.md` file is created in the workspace while the browser is open
- **AND** the user does not activate the refresh control
- **THEN** the tree does not yet list the new file

#### Scenario: The previewed file updates in place

- **WHEN** a file is selected in the browser and its contents are modified on disk
- **THEN** the preview re-renders with the updated content without user action
- **AND** the reading position is preserved and no loading indicator is presented

#### Scenario: Selecting a different file moves the watch

- **WHEN** the user selects a different file in the browser
- **THEN** the previously previewed file is no longer watched
- **AND** the newly selected file is

#### Scenario: Closing the browser releases the watch

- **WHEN** the file browser is closed or its browse root changes
- **THEN** no document watch remains registered for the file it was previewing

## ADDED Requirements

### Requirement: The Selected File Is Addressable

Selecting a file in the browser SHALL form an Address naming that file within its browse root, per the *File Addresses* requirement in the `view-routing` capability. The selection SHALL therefore be linkable, SHALL be reflected in the served UI's browser location, and SHALL be restored when the application loads at that address — opening the browser rooted at the named root with that file selected and previewed.

Selecting a file SHALL create a history entry, and selecting a different file SHALL create another, so that the back gesture returns to the previously previewed file. Expanding or collapsing a folder, and typing in the path filter, SHALL NOT form an Address or create a history entry, because neither changes which document is shown (see the *History Entry Discipline* requirement in the `view-routing` capability).

A file address that resolves to a root but names a file that no longer exists SHALL report not found while leaving the browser's tree usable, consistent with the *Empty and Error States* requirement.

#### Scenario: Selecting a file forms an address

- **WHEN** the user selects a `.md` file in the browser's folder tree
- **THEN** an Address naming that file within the browse root is formed

#### Scenario: A file address restores the browser and the selection

- **WHEN** the application loads at an address naming a file within a registered browse root
- **THEN** the file browser opens rooted at that root
- **AND** that file is selected and its markdown is previewed

#### Scenario: Back returns to the previously previewed file

- **WHEN** the user selects one file and then another
- **AND** invokes the back gesture
- **THEN** the first file is selected and previewed again

#### Scenario: Folder expansion forms no address

- **WHEN** the user expands or collapses a folder, or types in the path filter
- **THEN** no new Address is formed and no history entry is created

#### Scenario: An address naming a deleted file reports not found

- **WHEN** the application loads at a file address whose root resolves but whose file no longer exists
- **THEN** it reports not found
- **AND** the browser's folder tree remains usable

### Requirement: A Previewed File Opens in a Reader Window

A file row in the browser's folder tree SHALL be openable in a reader window by the launch gesture specified in the *Launching a Reader Window* requirement of the `reader-window` capability. Doing so SHALL NOT change the browser's own selection, its preview, or its scroll position.

#### Scenario: Opening a file row in a reader leaves the browser alone

- **WHEN** the user performs the reader launch gesture on a file row while a different file is selected
- **THEN** a reader window opens for the clicked file
- **AND** the browser's selection and preview are unchanged
