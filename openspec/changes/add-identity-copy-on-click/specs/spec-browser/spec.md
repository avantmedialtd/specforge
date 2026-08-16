## MODIFIED Requirements

### Requirement: Change Identity Header in the Detail Pane

While the detail pane's target is an OpenSpec artifact, the pane SHALL render a **change-identity header** above the artifact's markdown, naming the change the artifact belongs to. The header applies to the artifact target only: the commit detail view, the Dashboard, the workspace file browser, the Archive view, and the Settings view each carry their own header and SHALL be unaffected.

**Content.** The header SHALL display the change's **directory name** — the `openspec/changes/<name>` folder name, which is the identifier a user hands to external tooling — rendered verbatim and in full, with no truncation, ellipsis, or transformation. It SHALL NOT substitute the change's `proposal.md` title, which the tree already shows (see *Two-Line Sole-Change-Row Layout*) and which is not the change's filesystem identity. Following the name, the header SHALL show the owning worktree's branch as an outlined chip (per *visual-identity → Outlined Chip Badges*). When the artifact belongs to a flat (non-git) workspace, or the worktree's branch is otherwise not known, no chip SHALL be rendered and the header SHALL show the name alone. An **archived** change SHALL render no chip: it has no live worktree, and the worktree path its artifact is read from routinely hosts other, active changes whose branch was never the archived change's.

**Copy on click.** The change name is a **control**. A single primary click on it SHALL place exactly that name on the clipboard. What is copied SHALL be the name alone — never the branch chip's text, and never any surrounding whitespace. The same click SHALL also select the name atomically, so the selection serves as immediate confirmation of exactly what was copied.

The application SHALL perform the clipboard write itself. (This supersedes the previous contract, under which the application performed no clipboard write and the user completed the copy with the platform's own gesture; that contract was adopted under constraints of the tree pane, which do not apply to the detail pane.) Where the asynchronous Clipboard API is not exposed — a non-loopback bind on a plain-HTTP origin is not a secure context, see the `web-ui` capability — the application SHALL still copy, using a synchronous copy over the selection it has just made. The two mechanisms SHALL NOT be chained such that a failure of the first is what triggers the second, because the synchronous mechanism is only permitted inside the originating user gesture and awaiting the asynchronous one ends it.

**Confirmation.** The header SHALL confirm the outcome. A successful copy SHALL be indicated visually and announced to assistive technology; a refused copy SHALL be distinguished from a successful one, and SHALL leave the name selected so the platform's own copy shortcut still completes the action. Confirmation SHALL NOT change the layout of the header — no label substitution, no added or removed glyph — because the name shares a flex row with the branch chip and may wrap, so any width change would move the row on every copy. Confirmation SHALL revert on its own, and SHALL NOT outlive the artifact it described.

**Keyboard.** The change name SHALL be reachable by keyboard as a single tab stop within the detail pane, SHALL expose an accessible name describing the copy action and the value, and SHALL be activated by Enter and by Space, performing the same copy as a click. It SHALL show the application's standard focus indicator. No global keyboard chord SHALL be introduced for this; in particular the platform's own copy shortcut SHALL retain its native meaning everywhere. The tree's roving-focus, single-Tab-stop model SHALL be unaffected.

**Persistence while reading.** The header SHALL remain visible while the artifact's content scrolls, so the change's identity is answerable at any scroll position rather than only at the top of the document.

**Clearance of the native titlebar strip.** In the native desktop window on macOS, a drag region spans the full width of the top of the window (see *visual-identity → macOS Hidden Inset Titlebar Layout*), and a press inside it enters window drag or zoom rather than reaching what is beneath. The header SHALL be positioned so that the change name lies **clear of that region**, so a click on the name copies it rather than starting a window drag, and a double-click does not toggle window zoom. That clearance SHALL hold at **every scroll position**, not only at scroll top. The header's own background SHALL continue to span the full pane width across the cleared area, so no document content is visible above the identity at any scroll position. The drag region SHALL be left intact: the area the header clears SHALL remain draggable, and no exception SHALL be carved out of it. This clearance is a property of the native window only; the served web UI renders no such region and SHALL receive no offset.

**Anchoring.** Because the header occupies the top of the pane's scroll port, scroll anchors (see *Section and Task Scroll Anchors*) SHALL account for its height: a section or task scrolled to SHALL come to rest fully visible below the header, never underneath it. The height SHALL be taken from the rendered header rather than from a fixed constant, because the change name renders in full and therefore wraps at narrow pane widths — and because the macOS clearance changes that height. Any clearance SHALL therefore be inside the measured element.

**Placement.** The header SHALL be horizontally aligned with the artifact's prose column — sharing its width bound and horizontal origin — so it reads as heading the document rather than floating in the pane.

#### Scenario: Detail pane names the change it is rendering

- **WHEN** the user selects any artifact of a change in the tree
- **THEN** the detail pane displays that change's directory name above the rendered markdown
- **AND** the name is shown in full, with no truncation or ellipsis
- **AND** the name shown is the change's directory name, not its proposal title

#### Scenario: Branch appears as a chip beside the name

- **WHEN** the detail pane renders an artifact of a change in a git worktree on a named branch
- **THEN** the header shows that branch name as an outlined chip following the change name

#### Scenario: A flat-workspace artifact shows no branch chip

- **WHEN** the detail pane renders an artifact of a change in a flat (non-git) workspace
- **THEN** the header shows the change's directory name alone
- **AND** no branch chip is rendered

#### Scenario: An archived change shows no branch chip

- **WHEN** the detail pane renders an artifact of an archived change
- **AND** the worktree path it was read from hosts active changes on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the header

#### Scenario: One click copies the whole name

- **WHEN** the user clicks once on the change name in the header
- **THEN** exactly that change name is placed on the clipboard
- **AND** the name is also selected, as confirmation of what was copied
- **AND** a successful copy is indicated and announced

#### Scenario: The copied value excludes the branch

- **WHEN** the user clicks once on the change name of a change whose branch chip is displayed
- **THEN** the clipboard contains the change name only
- **AND** the branch chip's text is not part of what was copied, nor of the selection

#### Scenario: Copy works where the asynchronous clipboard API is unavailable

- **WHEN** the web UI is reached over a non-loopback bind on a plain-HTTP origin, where `navigator.clipboard` is not exposed
- **AND** the user clicks once on the change name
- **THEN** the name is still placed on the clipboard, by the synchronous mechanism over the selection
- **AND** no failure is reported to the user

#### Scenario: A refused copy leaves the value selected

- **WHEN** a copy is attempted and the clipboard write is refused
- **THEN** the failure is distinguished from a success, not reported as one
- **AND** the change name remains selected, so the platform's copy shortcut completes the action

#### Scenario: Confirming a copy does not move the header

- **WHEN** a copy succeeds and the header shows its confirmation
- **THEN** the change name occupies the same width as before the copy
- **AND** no element of the header changes position

#### Scenario: Keyboard copies without a chord

- **WHEN** the user moves focus to the change name and presses Enter or Space
- **THEN** the same copy occurs as for a click
- **AND** the focus indicator is visible on the name
- **AND** the platform's own copy shortcut retains its native meaning

#### Scenario: Identity survives scrolling a long artifact

- **WHEN** the user scrolls an artifact long enough that its first line leaves the viewport
- **THEN** the change-identity header remains visible

#### Scenario: An anchored section is not obscured by the header

- **WHEN** the user selects a section or task row that scrolls the artifact to that anchor
- **THEN** the anchored section or task comes to rest fully visible
- **AND** it is not positioned underneath the change-identity header, at any header height

#### Scenario: The change name is clickable in the native macOS window

- **WHEN** the application runs in the native window on macOS, where the titlebar drag region covers the top of the window
- **AND** the user clicks once on the change name
- **THEN** the name is copied
- **AND** the window does not begin a drag
- **AND** a double-click on the name does not toggle window zoom

#### Scenario: Clearance holds while the artifact is scrolled

- **WHEN** the application runs in the native window on macOS and the artifact is scrolled to any position
- **THEN** the change name remains clear of the titlebar drag region and remains clickable
- **AND** no document content is visible above the header

#### Scenario: The titlebar drag region keeps working

- **WHEN** the application runs in the native window on macOS
- **AND** the user presses in the top 32px of the window over the detail pane, outside the change name
- **THEN** the window enters native drag mode exactly as it does elsewhere along the strip

#### Scenario: The served web UI takes no titlebar offset

- **WHEN** the web UI is served in a browser, which renders no titlebar drag region
- **THEN** the header takes no clearance offset
- **AND** the identity sits at the top of the pane

#### Scenario: Non-artifact targets are unaffected

- **WHEN** the detail pane renders the Dashboard, a commit's detail view, the workspace file browser, the Archive view, or the Settings view
- **THEN** no change-identity header is rendered over it
- **AND** each of those views keeps the header it renders today
