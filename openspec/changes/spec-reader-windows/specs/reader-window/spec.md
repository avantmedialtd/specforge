# reader-window Delta — Reader Windows for Workspace Documents

## ADDED Requirements

### Requirement: Reader Window Surface

A **reader window** SHALL present exactly one markdown document and nothing that navigates. It SHALL render that document with the same renderer, and under the same guarantees, as the detail pane — see the *Markdown Rendering of Leaf Artifacts*, *Mermaid Diagram Rendering*, *SVG Fence Rendering*, *Mathematical Notation Rendering*, *Maximized Figure View*, and *Link Handling in Rendered Artifacts* requirements in the `spec-browser` capability — together with the document's identity, so the window states what it is showing.

A reader window SHALL NOT contain the workspace tree, the commit rail, the sidebar footer, the Settings or Archive entry points, the quota indicators, or any other affordance that changes which document is displayed. A reader window displays the document it was opened for, for its whole life.

A reader window SHALL be read-only, consistent with the *Read-Only Viewer* requirement in the `spec-browser` capability. Opening a document in a reader window SHALL NOT change what the main window is displaying, and SHALL NOT alter the main window's tree selection, scroll position, or pane visibility.

#### Scenario: A reader window shows only the document

- **WHEN** the user opens a document in a reader window
- **THEN** the window renders that document's markdown and its identity
- **AND** it contains no workspace tree, no commit rail, and no Settings or Archive entry point

#### Scenario: Rich content renders as it does in the pane

- **WHEN** a document opened in a reader window contains a `mermaid` fence, an `svg` fence, or display math
- **THEN** each renders exactly as it does in the detail pane, including the maximize affordance on a successfully rendered figure

#### Scenario: Opening a reader window leaves the main window alone

- **WHEN** the user opens a document in a reader window while the main window is displaying a different artifact
- **THEN** the main window continues to display that artifact
- **AND** its tree selection and scroll position are unchanged

#### Scenario: A reader window offers no way to another document

- **WHEN** a reader window is open
- **THEN** it presents no control, list, or link that would display a different document in that window

### Requirement: Launching a Reader Window

A document SHALL be openable in a reader window two ways: by a **gesture** on a row that names it, and by a **visible control** on the document being read. Both SHALL mint the same address and the same window title, so they are two spellings of one operation rather than two operations.

**The gesture.** A primary click held with the platform's command modifier — Cmd on macOS, Ctrl elsewhere — on an artifact row in the workspace tree or a file row in the workspace file browser's folder tree. The modifier SHALL be selected by platform rather than accepting either: on macOS a Ctrl-click IS the secondary click, so accepting it there would turn every attempt to reach a context menu into a new window.

The gesture SHALL NOT change the launching surface's own state: a Cmd/Ctrl-click SHALL NOT alter the tree's selection, SHALL NOT alter what the detail pane displays, and SHALL NOT create a history entry, because it navigates nothing (see the *History Entry Discipline* requirement in the `view-routing` capability).

A row that names no document — a grouping or disclosure-only node, a change row, or a folder — SHALL NOT open a reader window, because it has no document to show.

**The control.** Every surface that renders a document with an address SHALL present a control that opens that document in a reader window — the detail pane and the file browser's preview. A reader window itself SHALL NOT present one: its document is already detached, so the control would name an operation with nothing to do.

The control SHALL be operable by keyboard as well as by pointer (see the *Shell Keyboard Operability* requirement in the `spec-browser` capability). On a device that reports no hover capability it SHALL be rendered visibly at rest, and on a device whose primary pointer is coarse it SHALL present an enlarged hit area, per the *Essential Controls Are Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirements in the `touch-input` capability.

That last clause is not a courtesy. A modifier chord is invisible, and a device with no hover generally has no modifier key either — so a gesture-only feature would be not merely undiscoverable there but **unreachable**. The control is what makes reader windows exist on such a device at all.

Activating the control SHALL NOT alter the reading position, the selection, or the navigation history of the surface it sits on: it detaches what is already displayed rather than navigating to it.

#### Scenario: Cmd-click on an artifact row opens a reader window

- **WHEN** the user Cmd/Ctrl-clicks an artifact row in the workspace tree
- **THEN** that artifact opens in a reader window

#### Scenario: Cmd-click on a file row opens a reader window

- **WHEN** the user Cmd/Ctrl-clicks a file row in the workspace file browser's folder tree
- **THEN** that file opens in a reader window

#### Scenario: The launching surface is undisturbed

- **WHEN** the user Cmd/Ctrl-clicks a row while a different artifact is selected and displayed
- **THEN** a reader window opens for the clicked row's document
- **AND** the tree's selection, the detail pane's contents, and the navigation history are all unchanged

#### Scenario: A non-document row opens nothing

- **WHEN** the user Cmd/Ctrl-clicks a grouping row, a change row, or a folder row
- **THEN** no reader window opens

#### Scenario: The secondary click is not the launch gesture on macOS

- **WHEN** the user Ctrl-clicks a row on macOS, which is that platform's secondary click
- **THEN** no reader window opens

#### Scenario: The document being read offers a control

- **WHEN** the detail pane or the file browser's preview is rendering a document
- **THEN** a control is present that opens that document in a reader window
- **AND** activating it opens the same window the launch gesture on that document's row would

#### Scenario: A reader window offers no control of its own

- **WHEN** a reader window is rendering its document
- **THEN** it presents no control to open that document in a reader window

#### Scenario: The control is reachable without hover or a modifier key

- **WHEN** the application is displayed on a device that reports no hover capability
- **THEN** the control is rendered visibly at rest rather than revealed on hover
- **AND** on a device whose primary pointer is coarse it presents an enlarged hit area

#### Scenario: Activating the control disturbs nothing

- **WHEN** the user activates the control while scrolled away from the top of the document
- **THEN** a reader window opens for that document
- **AND** the surface's reading position, selection, and navigation history are unchanged

### Requirement: One Reader Window Per Document

A reader window SHALL be identified by the document it displays. Requesting a reader window for a document that already has one SHALL bring the existing window to the front and focus it, rather than opening a second window on the same document. Two distinct documents SHALL be able to have reader windows open at the same time.

Because a document's identity is its address, two addresses that name the same file SHALL resolve to the same reader window (see the *Shortest Unambiguous Address* requirement in the `view-routing` capability).

#### Scenario: Re-opening the same document focuses the existing window

- **WHEN** a reader window is already open for a document
- **AND** the user performs the open gesture for that same document again
- **THEN** the existing reader window is brought to the front and focused
- **AND** no second window is created

#### Scenario: Different documents get different windows

- **WHEN** the user opens two different documents in reader windows
- **THEN** both windows are open at the same time, each showing its own document

### Requirement: Reader Window Title and Titlebar

A reader window SHALL carry a native titlebar, rather than the overlay titlebar treatment the main window uses, so that the window's title is legible and the window participates in the operating system's own window management. On macOS this places the reader window in the Window menu's window list and makes it reachable by the system's window-cycling shortcut.

The title SHALL name the document: its file name, followed by the context that disambiguates it — the change or capability it belongs to, and its workspace. The title SHALL remain stable while the document's contents change.

The main window's own titlebar treatment SHALL be unchanged, and the traffic-light clearance the main window's layout reserves SHALL NOT be applied to a reader window, which does not need it.

#### Scenario: A reader window has a native titlebar

- **WHEN** a reader window opens
- **THEN** it presents a native titlebar showing the document's title
- **AND** the main window's overlay titlebar treatment is unchanged

#### Scenario: The reader appears in the system window list

- **WHEN** a reader window is open on macOS
- **THEN** it is listed in the Window menu's window list alongside the main window

#### Scenario: Title survives a content change

- **WHEN** a reader window's document is rewritten on disk
- **THEN** the window's title is unchanged

### Requirement: Shared Reader Window Geometry

Reader windows SHALL share one remembered size, which SHALL persist across application restarts. Resizing any reader window SHALL update it, and the next reader window to open SHALL adopt it. A reader window opening while another reader window is already visible SHALL be offset from it, so windows stack visibly rather than landing exactly on top of one another.

Geometry SHALL NOT be remembered per document. The application SHALL NOT accumulate persisted window state keyed by the documents a user has opened.

#### Scenario: A resized reader sets the size for the next one

- **WHEN** the user resizes a reader window and then opens a reader window for a different document
- **THEN** the new window opens at the resized dimensions

#### Scenario: Reader size survives a restart

- **WHEN** the user resizes a reader window, quits SpecForge, relaunches it, and opens a reader window
- **THEN** the window opens at the previously set size

#### Scenario: A second reader is offset from the first

- **WHEN** a reader window is visible and the user opens a reader window for a different document
- **THEN** the new window is positioned offset from the visible one rather than exactly over it

#### Scenario: Opening many documents accumulates no per-document state

- **WHEN** the user has opened reader windows for many different documents over time
- **THEN** the application's persisted window state contains no per-document entry for any of them

### Requirement: Dismissing a Reader Window Destroys It

A reader window SHALL be dismissable by the platform's close-window shortcut, by the Escape key, and by its own titlebar close control. Dismissal SHALL destroy the window and release the resources it holds, including its document watch (see the *Watch Registration Is Reference-Counted* requirement in the `document-watch` capability).

This is deliberately the opposite of the main window, whose close button hides it so that the tray indicator and the filesystem watcher keep running. That asymmetry SHALL be preserved: closing a reader window SHALL NOT hide it, and SHALL NOT be reachable by any path that would leave an invisible reader window alive.

Escape SHALL dismiss the reader window only when nothing else in it has claimed the key. A maximized figure open in a reader window SHALL consume Escape first, so one Escape returns to the document and a second closes the window (see the *Maximized Figure View* requirement in the `spec-browser` capability).

Closing every reader window SHALL NOT quit SpecForge, and SHALL NOT affect the main window's visibility.

#### Scenario: Close destroys rather than hides

- **WHEN** the user closes a reader window
- **THEN** the window is destroyed
- **AND** no hidden reader window for that document remains

#### Scenario: Escape closes the reader

- **WHEN** a reader window has focus and no maximized figure is open in it
- **AND** the user presses Escape
- **THEN** the reader window closes

#### Scenario: Escape dismisses a maximized figure before the window

- **WHEN** a reader window has a maximized figure open
- **AND** the user presses Escape
- **THEN** the maximized figure closes and the reader window remains open
- **AND** a second Escape closes the reader window

#### Scenario: Closing readers does not quit the application

- **WHEN** the user closes every open reader window while the main window is hidden
- **THEN** SpecForge continues running and its tray indicator remains available

### Requirement: Reader Windows Stay Fresh

A reader window SHALL re-read and re-render its document when that document changes on disk, without user action, for any document it can display — not only for documents beneath a change directory. It SHALL obtain this through a document watch (see the `document-watch` capability), registered when the window opens and released when it closes.

A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT re-render the document when its bytes are unchanged, exactly as specified for the detail pane by the *Reactive Updates from Filesystem* requirement in the `spec-browser` capability. A reader window is a document surface, and every guarantee that requirement makes about a document surface binds it.

#### Scenario: A parked reader updates while unfocused

- **WHEN** a reader window is open and does not have focus
- **AND** its document is rewritten on disk
- **THEN** the window re-renders with the updated content without being focused or clicked

#### Scenario: A main-spec document updates live

- **WHEN** a reader window is open on a capability specification under the workspace's `openspec/specs/` directory
- **AND** that file is modified on disk
- **THEN** the window re-renders with the updated content

#### Scenario: A document outside the OpenSpec directory updates live

- **WHEN** a reader window is open on a markdown file that lies outside the workspace's `openspec/` directory
- **AND** that file is modified on disk
- **THEN** the window re-renders with the updated content

#### Scenario: Reading position survives a background update

- **WHEN** a reader window is scrolled away from the top and its document is rewritten on disk
- **THEN** the updated content is rendered
- **AND** the reading position is preserved and no loading indicator is presented

### Requirement: A Vanished Document Is Reported, Not Followed

When the document a reader window displays is deleted, renamed away, or moved — including by its change being archived — the window SHALL continue to display the content it already holds, SHALL indicate that the document is no longer present at the address it was opened for, and SHALL NOT close itself.

A reader window SHALL NOT navigate to a different file on the reader's behalf, including to an archived copy of the same document. The window displays the address it was opened for, and a window that silently began displaying a different address would contradict the *Reader Window Surface* requirement's guarantee that it never changes what it shows.

If the document reappears at the same address, the window SHALL resume rendering it and SHALL clear the indication.

#### Scenario: Deleting the document leaves the last content visible

- **WHEN** a reader window is open and its document is deleted from disk
- **THEN** the window continues to display the content it already loaded
- **AND** it indicates that the document is no longer present
- **AND** it does not close

#### Scenario: Archiving a change does not redirect the reader

- **WHEN** a reader window is open on an artifact of an active change
- **AND** that change is archived, moving the artifact into the archive directory
- **THEN** the window indicates that the document is no longer present at its address
- **AND** it does not begin displaying the archived copy

#### Scenario: A restored document resumes

- **WHEN** a reader window is indicating that its document is no longer present
- **AND** a file reappears at that same address
- **THEN** the window renders it and clears the indication

### Requirement: Reader Presentation Is Not Part of the Address

Whether a document is presented in a reader window or in the main window's detail pane SHALL NOT be encoded in its Address. The Address names the document; the reader is a presentation of it. The pure Address-to-URL codec SHALL be unchanged by this capability — it SHALL gain no reader parameter, no reader variant, and no additional argument (see the *Address and URL Round-Trip Through a Pure Codec* requirement in the `view-routing` capability).

In the served web UI, where the address is reflected in the browser location, the reader presentation SHALL be carried outside the path that the codec reads, so that the same URL path denotes the same document whether it is opened as a reader or in the full application.

#### Scenario: The codec is unchanged by the reader

- **WHEN** an address is encoded to a path and decoded back
- **THEN** the round trip is identical whether or not that document is currently open in a reader window
- **AND** the encoded path contains no reader marker

#### Scenario: A reader URL and a shell URL share a path

- **WHEN** the served web UI opens a document as a reader and the same document in the full application
- **THEN** both URLs carry the same path
- **AND** they differ only outside the path

### Requirement: Reader Windows Are a Windowed-Host Capability

Reader windows SHALL be available in the SpecForge desktop application and in the served web UI, which both have a window system to open one in. In the served web UI a reader SHALL be opened as a browser window or tab identified by the document, so that requesting a reader for a document that already has one reuses and focuses it rather than opening a second.

The terminal frontend SHALL NOT provide reader windows and SHALL be unaffected by this capability. Its behaviour is unchanged.

#### Scenario: The desktop application opens a native reader window

- **WHEN** the user performs the open gesture in the SpecForge desktop application
- **THEN** a native reader window opens for that document

#### Scenario: The served web UI opens a reader window

- **WHEN** the user performs the open gesture in the served web UI
- **THEN** a browser window or tab opens showing that document as a reader
- **AND** repeating the gesture for the same document reuses and focuses it

#### Scenario: The terminal frontend is unaffected

- **WHEN** the terminal frontend is run
- **THEN** it presents no reader-window affordance and behaves exactly as before
