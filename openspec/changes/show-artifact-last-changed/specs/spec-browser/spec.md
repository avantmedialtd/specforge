## MODIFIED Requirements

### Requirement: Change Identity Header in the Detail Pane

While the detail pane's target is an OpenSpec artifact, the pane SHALL render a **change-identity header** above the artifact's markdown, naming the change the artifact belongs to. The header applies to the artifact target only: the commit detail view, the Dashboard, the workspace file browser, the Archive view, and the Settings view each carry their own header and SHALL be unaffected.

**Content.** The header SHALL display the change's **directory name** — the `openspec/changes/<name>` folder name, which is the identifier a user hands to external tooling — rendered verbatim and in full, with no truncation, ellipsis, or transformation. It SHALL NOT substitute the change's `proposal.md` title, which the tree already shows (see *Two-Line Sole-Change-Row Layout*) and which is not the change's filesystem identity. Following the name, the header SHALL show the owning worktree's branch as an outlined chip (per *visual-identity → Outlined Chip Badges*). When the artifact belongs to a flat (non-git) workspace, or the worktree's branch is otherwise not known, no chip SHALL be rendered and the header SHALL show the name alone. An **archived** change SHALL render no chip: it has no live worktree, and the worktree path its artifact is read from routinely hosts other, active changes whose branch was never the archived change's.

**Branch chip colour.** The branch chip SHALL be **tinted to the owning workspace's palette colour** — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour — so the artifact under the header reads as belonging to the workspace it came from. The workspace whose colour applies is the one owning the worktree the artifact was read from, which is the same worktree whose branch the chip names; no other workspace's colour SHALL be substituted. When the owning workspace has no configured palette colour, the chip SHALL render in the neutral ink it renders in today, and SHALL NOT fall back to an arbitrary or derived colour.

The header's chip and the tree's chip naming the same branch of the same change SHALL render **identically** — the same tint, weight, and treatment (see the *Two-Line Sole-Change-Row Layout* requirement, which specifies the tree's chip). The two surfaces are visible simultaneously, so a single value SHALL NOT be presented two ways. This equivalence is a property of the rendered result and SHALL hold for every palette colour and for the untinted case, so that changing how one surface renders the chip cannot leave the other behind.

**Last changed.** The header SHALL report **when the artifact currently rendered last changed**, as an interval elapsed since that moment, expressed in relative terms (for example `just now`, `9 min ago`, `12 days ago`) rather than as an absolute clock time. The detail pane is already refreshed live (see *Reactive Updates from Filesystem*), so this value does not report whether the view is current; it reports how long the artifact has stood.

The value SHALL be the modification time of **the artifact's own file** — not of the change's directory, and not of any sibling artifact. A write to `tasks.md` SHALL NOT be reported as a change to the `proposal.md` on screen, because the two are edited independently and reporting the directory's newest write would be wrong in exactly the case a reader is most likely to be watching.

Where **no** modification time is available for the artifact's file, the header SHALL render no label at all, and SHALL NOT substitute a default, derived, or epoch time in its place. The artifact itself SHALL still be displayed: an unreadable timestamp is not a failed read, and a reader SHALL NOT lose the document because the application could not date it. This mirrors the branch chip, which is likewise absent rather than defaulted when there is no branch to name.

The value SHALL be a filesystem modification time, and the header SHALL claim no more of it than that. Any operation that writes the file sets it, including a clone, a checkout, or a branch switch — so on a freshly cloned repository every artifact SHALL report having last changed at the time of that clone, regardless of when it was genuinely edited. This SHALL hold **uniformly for every artifact**, active and archived alike: no class of artifact SHALL substitute a different source for this value, because the property belongs to modification times in general and not to any one class, and an exception carved for one class would imply the others are trustworthy in a way they are not.

The label SHALL **advance without user action** for as long as the artifact remains on screen, so a reader who has not navigated away is never shown an interval that stopped counting when the pane was painted. It SHALL be updated at a cadence no finer than the smallest unit it displays. An elapsed interval that would be negative — a file whose modification time lies in the future, which clock skew, restored archives, and network filesystems all produce — SHALL be presented as the present moment rather than as a future time. Whatever advances the label SHALL NOT outlive the artifact it described.

The label SHALL occupy a **constant width**, sized to the widest text it can display, so that neither its advancing nor a change of unit alters the layout of the header. The change name shares a single flex row with the branch chip and yields to width pressure by re-wrapping mid-identifier; a label whose box changed size as it advanced would therefore re-lay-out the change name on a timer, unprompted. This is the same defect the *Confirmation* clause below forbids for a click-driven width change, arriving from a trigger the reader did not initiate at all.

**Copy on click.** The change name is a **control**. A single primary click on it SHALL place exactly that name on the clipboard. What is copied SHALL be the name alone — never the branch chip's text, never the last-changed label, and never any surrounding whitespace. The same click SHALL also select the name atomically, so the selection serves as immediate confirmation of exactly what was copied.

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

#### Scenario: The branch chip carries the workspace's palette colour

- **WHEN** the detail pane renders an artifact of a change whose owning workspace has a configured palette colour
- **THEN** the branch chip's text and border render in a contrast-safe shade of that colour
- **AND** the colour is the one configured for the workspace owning the worktree the artifact was read from

#### Scenario: The branch chip stays neutral when no palette colour is configured

- **WHEN** the detail pane renders an artifact of a change whose owning workspace has no configured palette colour
- **THEN** the branch chip renders in the neutral ink it renders in today
- **AND** no colour is derived or substituted in place of the missing one

#### Scenario: The header chip and the tree chip agree

- **WHEN** the tree and the detail pane are both visible, and each shows a chip naming the same branch of the same change
- **THEN** the two chips render identically, in the same tint and treatment
- **AND** this holds for every palette colour and for a workspace with none configured

#### Scenario: A flat-workspace artifact shows no branch chip

- **WHEN** the detail pane renders an artifact of a change in a flat (non-git) workspace
- **THEN** the header shows the change's directory name alone
- **AND** no branch chip is rendered

#### Scenario: An archived change shows no branch chip

- **WHEN** the detail pane renders an artifact of an archived change
- **AND** the worktree path it was read from hosts active changes on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the header
- **AND** no palette colour is applied, there being no chip to tint

#### Scenario: The header reports when the artifact last changed

- **WHEN** the detail pane renders an artifact whose file was last written some interval ago
- **THEN** the header displays that interval in relative terms
- **AND** the interval is measured from the modification time of that artifact's own file

#### Scenario: A sibling artifact's edit is not reported as this one's

- **WHEN** the detail pane is rendering a change's `proposal.md`
- **AND** `tasks.md` in the same change directory is written
- **THEN** the interval reported for the `proposal.md` on screen is unchanged
- **AND** it continues to reflect when `proposal.md` itself was last written

#### Scenario: The label advances while the reader stays on the artifact

- **WHEN** the detail pane has rendered an artifact and enough time passes for the reported interval to change
- **AND** the user has neither navigated away nor taken any action
- **AND** nothing on disk has changed
- **THEN** the displayed interval advances to reflect the time now elapsed

#### Scenario: A rewrite with identical bytes still updates the label

- **WHEN** the detail pane is rendering an artifact
- **AND** that artifact's file is rewritten with content identical to what is displayed, moving its modification time
- **THEN** the reported interval updates to reflect the new modification time
- **AND** the rendered document and the reading position are unchanged

#### Scenario: The advancing label never moves the change name

- **WHEN** the reported interval advances, including across a change of unit
- **THEN** the label occupies the same width as before
- **AND** the change name occupies the same width and wraps at the same points
- **AND** no element of the header changes position

#### Scenario: A modification time in the future is not shown as future

- **WHEN** the detail pane renders an artifact whose file carries a modification time later than the present
- **THEN** the header presents the artifact as having changed at the present moment
- **AND** no future interval is displayed

#### Scenario: An artifact with no readable modification time shows no label

- **WHEN** the detail pane renders an artifact whose file reports no usable modification time
- **THEN** the artifact's markdown is displayed as normal
- **AND** no last-changed label is rendered
- **AND** no default, derived, or epoch time is shown in its place

#### Scenario: An archived artifact reports its modification time like any other

- **WHEN** the detail pane renders an artifact of an archived change
- **THEN** the header reports that file's modification time under the same rule as an active change's
- **AND** no alternative source, such as the archive date in the directory name, is substituted

#### Scenario: The label stops when the artifact it described is gone

- **WHEN** the detail pane's artifact target changes or clears while the label is advancing
- **THEN** the label ceases to advance for the artifact that is no longer rendered
- **AND** no update is applied to a header describing a different artifact

#### Scenario: One click copies the whole name

- **WHEN** the user clicks once on the change name in the header
- **THEN** exactly that change name is placed on the clipboard
- **AND** the name is also selected, as confirmation of what was copied
- **AND** a successful copy is indicated and announced

#### Scenario: The copied value excludes the branch

- **WHEN** the user clicks once on the change name of a change whose branch chip is displayed
- **THEN** the clipboard contains the change name only
- **AND** the branch chip's text is not part of what was copied, nor of the selection

#### Scenario: The copied value excludes the last-changed label

- **WHEN** the user clicks once on the change name while the last-changed label is displayed
- **THEN** the clipboard contains the change name only
- **AND** the label's text is not part of what was copied, nor of the selection

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

### Requirement: Reactive Updates from Filesystem

The tree pane and the detail pane SHALL reflect on-disk changes within the watcher's debounce window without requiring user action. After the watcher finishes processing a debounced batch of filesystem events, the *first* refresh the frontend performs in response to that batch SHALL observe the post-batch state — the UI MUST NOT lag behind by one event for any on-disk change, including content-only changes inside a change directory that is already tracked (artifact file creation, task checkbox toggles, edits to spec or proposal markdown).

The detail pane's refresh SHALL re-read the artifact it is currently rendering. It SHALL be driven by the change notification alone and MUST NOT be conditioned on the workspace named in that notification's payload, because a notification MAY carry any tracked workspace as a carrier rather than the workspace whose contents changed.

A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT alter the rendered document when the artifact's bytes are unchanged. (This narrows a previous contract, under which such a refresh was required to be wholly unobservable when the bytes were unchanged. That is no longer correct: a file rewritten with identical bytes has a **new modification time**, and the header reports modification time — see *Change Identity Header in the Detail Pane*, "Last changed". The protection this clause exists to give — an undisturbed reader, no loading indicator, no repaint of the document — is unchanged; what narrows is its scope, from the whole pane to the document within it.) When such a refresh fails to read the artifact, the pane SHALL continue to display the content it already holds rather than replacing it with an error. A read the user initiated by selecting an artifact retains its existing loading and error presentation.

The cost of the unfiltered subscription SHALL remain bounded by this guarantee: a refresh that changes neither the artifact's bytes nor its modification time SHALL do no work beyond the read itself, and one that changes only the modification time SHALL NOT re-render the document.

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

#### Scenario: Refresh with unchanged content does not repaint the document

- **WHEN** the detail pane is rendering an artifact
- **AND** a filesystem change elsewhere triggers a refresh whose read returns content identical to what is displayed, with an unchanged modification time
- **THEN** the rendered output, the reading position, and the loading indicator are all unchanged

#### Scenario: A modification-time-only change updates the header and nothing else

- **WHEN** the detail pane is rendering an artifact
- **AND** a refresh returns content identical to what is displayed but a newer modification time
- **THEN** the header's last-changed label updates
- **AND** the rendered document is not re-rendered
- **AND** the reading position is preserved and no loading indicator is presented
