## MODIFIED Requirements

### Requirement: File Sections

Each file SHALL render as a **section**: a header, then the file's content. The header SHALL stick to the top of the view while the reader scrolls through its section, and SHALL show:

- the file's path, or, for a renamed file, its old path, an arrow and its new path (`old → new`);
- its status;
- its old and new modes, whenever they differ;
- its counts, where the source gives them;
- a toggle that collapses and expands the section.

A renamed file whose content also changed SHALL show its hunks below that header.

Every line SHALL show its line numbers in a gutter (in unified, its old and new numbers side by side; side by side, one number beside each column), and every added or removed line its marker, `+` or `−`, in both layouts, so additions and removals are never conveyed by colour, or by column, alone. Added and removed lines SHALL also carry a background tint (see the *Syntax Highlighting* requirement).

Collapse SHALL be view state held by the diff view and kept per file. It SHALL NOT be a preference and SHALL NOT be persisted. A host MAY ask the view to collapse a section once (see the *Diff View Hosts* requirement). The collapse that follows SHALL be the same view state the header's toggle sets: the toggle SHALL expand the section again, and the host SHALL NOT hold, restore or repeat it unless it asks again.

#### Scenario: A renamed file's header names both paths

- **WHEN** a file renamed from `docs/a.md` to `docs/b.md` with one line changed is shown
- **THEN** its header shows `docs/a.md → docs/b.md`, its renamed status and its counts
- **AND** its section shows the hunk that changes the line

#### Scenario: A mode change is shown in the header

- **WHEN** an edited file's mode changes from `100644` to `100755`
- **THEN** its header shows its modified status and both modes

#### Scenario: The header stays in view

- **WHEN** the reader scrolls through a long section
- **THEN** its header stays visible at the top of the view until the section has scrolled past

#### Scenario: Markers are kept in both layouts

- **WHEN** a hunk holding a removed and an added line is shown in unified and then side by side
- **THEN** in both layouts the removed line carries a `−` marker and the added line a `+` marker, as well as its tint

#### Scenario: A section collapses and is not remembered

- **WHEN** the reader collapses a section with its header's toggle
- **THEN** the section's lines are hidden and its header remains
- **AND** nothing about the collapse is written to the application settings or to the surface's storage

#### Scenario: A host's collapse is the reader's to undo

- **WHEN** a host asks the view to collapse a file's section, and the reader then expands it with its header's toggle
- **THEN** the section collapsed when the host asked, and is expanded after the toggle
- **AND** it stays expanded until the host asks again

### Requirement: Selection and Copying

Side by side, a selection SHALL stay in the column it started in. A primary-button pointer-down in a side-by-side code cell SHALL name that cell's side for the whole view, and every file's grid SHALL then refuse selection in the other column, so a drag into the next file stays on the same side. The side SHALL stay named until a primary-button pointer-down outside a code cell, or until a primary-button pointer-up leaves the selection collapsed or outside the view; the collapsed selection a pointer-down leaves before a drag SHALL NOT clear it, and a secondary-button press, as for a context menu's Copy, SHALL leave it unchanged.

Copying a selection whose two ends lie in code cells of one file, while unified is in effect or a side is named, SHALL put text built from the model on the clipboard, never text read from the page: side by side, the named side's selected lines; in unified, the selected lines in the order shown. The lines of a hunk folded between the two ends (see the *Folded Hunks* requirement) are not shown, and SHALL be left out. That text SHALL be code text only, with no line numbers, markers, fillers or badges, and SHALL honour a partial first and last line.

Any other selection (one with an end in a file header, in a preamble such as a review thread, or in a hunk row; one spanning files; or, side by side, one made while no side is named) SHALL copy its text in document order, still leaving out line numbers, markers, fillers and badges. A folded hunk's heading row is a hunk row, and the controls a folded hunk or a hunk slot adds SHALL be left out of every copy.

Every copy SHALL yield the file's real characters: a character the view shows as an escape (see the *Hidden Characters Are Shown* requirement) SHALL be copied as itself, never as its escape. These guarantees SHALL hold in the desktop application on every platform and in the browser skin.

#### Scenario: A drag stays in its column

- **WHEN** side by side is in effect and the reader drags from an old-side code cell down across several paired rows
- **THEN** only old-side code is selected

#### Scenario: The named side carries into the next file

- **WHEN** the reader starts a drag in a new-side code cell of one file and continues it into the next file
- **THEN** the selection in the next file is on its new side only

#### Scenario: Copying one side yields that side's code

- **WHEN** side by side is in effect and the reader selects from the middle of old line 10 to the middle of old line 12 of one file and copies
- **THEN** the clipboard holds the old side's code from the selection's start in line 10 to its end in line 12, line by line
- **AND** it holds no line number, marker, filler, badge or new-side text

#### Scenario: Copying in unified yields the lines as shown

- **WHEN** unified is in effect and the reader selects a removed line and the added line below it in one file and copies
- **THEN** the clipboard holds both lines' code text in the order shown, without their numbers or markers

#### Scenario: A selection reaching a header copies in document order

- **WHEN** the reader selects from a file's header into its first lines and copies
- **THEN** the clipboard holds the selected text in document order, without line numbers, markers, fillers or badges

#### Scenario: A click ends the named side

- **WHEN** a side is named and the reader clicks a code cell without dragging
- **THEN** once the pointer is released no side is named, and either column accepts the next selection

#### Scenario: A selection without a named side copies in document order

- **WHEN** side by side is in effect, no side is named, and the reader selects the whole page from the keyboard and copies
- **THEN** the clipboard holds the text in document order, without line numbers, markers, fillers or badges

#### Scenario: Escaped characters copy as themselves

- **WHEN** the reader copies a line holding a zero-width space that the view shows as an escape
- **THEN** the clipboard holds the zero-width space itself, not the escape

#### Scenario: A folded hunk inside a selection is not copied

- **WHEN** unified is in effect, the second of a file's three hunks is folded, and the reader selects from a line of the first hunk to a line of the third and copies
- **THEN** the clipboard holds the selected lines of the first and third hunks
- **AND** it holds none of the second hunk's lines, and no text of its controls

### Requirement: Side-Qualified Line Identity and Switching

Every rendered line SHALL carry a side-qualified identity, in both layouts: old line n for a removed line, new line n for an added line, and both for a context line. The identity SHALL NOT depend on the layout, so content anchored to a side and a line number, as GitHub's `diffSide` and BitBucket's `inline.from` and `inline.to` give one, finds its line in either layout.

Switching the layout SHALL keep the reader's place by the identity of the topmost visible line: the line at the top of the view before the switch SHALL be at the top after it. When the topmost visible row is a folded hunk's heading, that heading SHALL be the topmost visible row after the switch. A switch SHALL keep, per file, every section's collapse, every withheld file already loaded with its hunks, every hunk the host folds, every folded hunk the reader showed, and the navigator's mark. A switch SHALL re-read nothing, invoking no command and starting no `git` process, and SHALL re-tokenise nothing: each hunk's rows and token lines are computed once and reused by both layouts.

#### Scenario: A context line carries both identities

- **WHEN** a context line is old line 12 and new line 14
- **THEN** it is found as old line 12 and as new line 14, in both layouts

#### Scenario: A switch keeps the reader's place

- **WHEN** removed old line 40 of a file is the topmost visible line in unified and the reader switches to side by side
- **THEN** the row holding old line 40 is the topmost visible row

#### Scenario: A switch back keeps the reader's place

- **WHEN** side by side is in effect, the topmost visible row holds the context line that is old line 40 and new line 42, and the reader switches to unified
- **THEN** that context line is the topmost visible line

#### Scenario: A switch keeps collapse, loaded files and the mark

- **WHEN** the reader has collapsed one section, loaded one withheld file and is reading a third file, and switches the layout
- **THEN** the collapsed section is still collapsed, the loaded file still shows its hunks, and the navigator still marks the third file

#### Scenario: A switch re-reads and re-tokenises nothing

- **WHEN** the reader switches the layout
- **THEN** no command is invoked and no `git` process is started
- **AND** no hunk is highlighted again

#### Scenario: A switch keeps folds and shown hunks

- **WHEN** the host folds two hunks of a file, the reader has shown one of them, and the reader switches the layout
- **THEN** the other hunk is still folded and the shown one still shows its lines

#### Scenario: A switch keeps a folded heading at the top

- **WHEN** a folded hunk's heading is the topmost visible row in unified and the reader switches to side by side
- **THEN** that heading is the topmost visible row

### Requirement: Diff View Hosts

The diff view SHALL be the one view through which every host shows a diff, such as commit detail or a pull request's view, so no host forks the renderer. A host SHALL pass it the model, the names of the two sides and a loader. Optional slots SHALL carry everything else a host adds, two per file and two per hunk:

- the **header extra**, rendered inside the file's sticky header, where a pull request's "viewed" mark would sit;
- the **preamble**, rendered between the header and the first hunk, across the section's full width, where a pull request's review threads would sit;
- the **hunk heading extra**, rendered in a hunk's heading row, in the gutter left of its `@@` heading, where a pull request's hunk mark would sit;
- the **hunk end**, rendered as a row after a hunk's last line, across the section's full width, only when the host gives it content for that hunk.

No slot SHALL be given a column, and each SHALL render the same in either layout. Beside its slots, a host MAY name the hunks it folds (see the *Folded Hunks* requirement), and MAY ask the view to collapse a file's section once (see the *File Sections* requirement). A host that passes no hunk slots and folds nothing SHALL see its hunks exactly as before: commit detail passes none.

The loader SHALL be offered only for a withheld file; a too-large or binary file SHALL show its state and no control.

The view SHALL show the two side names in its toolbar while side by side is in effect, with the escapes the *Hidden Characters Are Shown* requirement gives side names. Commit detail SHALL name the old side by the first parent's abbreviated id, or "empty tree" for a root commit, and the new side by the commit's abbreviated id.

#### Scenario: Slots render the same in both layouts

- **WHEN** a host fills a file's header extra with a mark and its preamble with a note, and the reader switches between the layouts
- **THEN** in both layouts the mark sits in the file's sticky header and the note spans the section's full width between the header and the first hunk

#### Scenario: Only a withheld file offers the loader

- **WHEN** a diff holds a withheld, a too-large and a binary file
- **THEN** only the withheld file offers "Load diff"
- **AND** the other two show their states with no control

#### Scenario: Commit detail names its sides

- **WHEN** commit detail shows side by side a commit whose first parent is `abc1234`
- **THEN** the toolbar names the old side `abc1234` and the new side by the commit's abbreviated id
- **AND** in unified, the toolbar shows neither name

#### Scenario: A root commit is compared with the empty tree

- **WHEN** commit detail shows a root commit side by side
- **THEN** the toolbar names the old side "empty tree"

#### Scenario: Hunk slots render the same in both layouts

- **WHEN** a host fills a hunk's heading extra with a mark and its end with a row, and the reader switches between the layouts
- **THEN** in both layouts the mark sits in the gutter left of the hunk's `@@` heading and the row follows the hunk's last line across the section's full width

#### Scenario: A hunk end the host leaves empty renders no row

- **WHEN** a host gives a hunk's end no content
- **THEN** no row follows that hunk's last line

#### Scenario: Commit detail shows no hunk controls

- **WHEN** commit detail shows a commit
- **THEN** no hunk carries a heading extra or an end row, and no hunk is folded

## ADDED Requirements

### Requirement: Folded Hunks

A host MAY fold any of a file's hunks (see the *Diff View Hosts* requirement). A folded hunk SHALL render as one row: the content of its heading extra, its `@@` heading, and a control that shows its lines and says how many there are ("Show 12 lines"). None of its lines and no end row SHALL render while it is folded.

**Showing.** Activating a folded hunk's control SHALL render the hunk in full, with its heading row's control then hiding it again ("Hide"). Which folded hunks the reader showed SHALL be view state held by the view, kept per file and hunk. It SHALL NOT be a preference and SHALL NOT be persisted. It SHALL be dropped when the host passes a new model. A hunk's shown state SHALL also be dropped whenever the host's fold of that hunk changes, so a hunk the host folds again folds.

**Keeping the reader's place.** Folding or showing a hunk, and collapsing a section at the host's request, SHALL keep one anchor where it was in the view:

| The change came from | The anchor |
|---|---|
| a control in a hunk's heading row, or the heading extra's content | that heading row |
| the hunk's end row | the first row after the hunk |
| the file's header, or a collapse the host asked for | the section's header, at the top of the view when it was stuck there |
| anything else | the topmost visible line; when that line's hunk folds, the hunk's heading row; when its section collapses, its header |

**Cost.** Folding, showing and hiding SHALL invoke no command, start no `git` process and re-tokenise nothing.

**Accessibility.** The show control SHALL be a button that exposes whether the hunk is shown through its expanded state, and SHALL be named by the number of lines and the hunk's first line identity ("Show 12 lines from new line 143"). A folded row SHALL read in visual order: the heading extra's content, the heading, the control.

#### Scenario: A folded hunk is one row

- **WHEN** a host folds the second of a file's three hunks, which has 12 lines
- **THEN** that hunk renders as one row holding its heading extra, its `@@` heading and a "Show 12 lines" control
- **AND** none of its 12 lines render

#### Scenario: A shown hunk is not remembered

- **WHEN** the reader shows a folded hunk and the host then passes a new model in which that hunk is still folded
- **THEN** the hunk is folded
- **AND** nothing about the show was written to the application settings or to the surface's storage

#### Scenario: A hunk the host folds again folds

- **WHEN** the reader showed a folded hunk, the host stopped folding it, and the host then folds it again
- **THEN** the hunk is folded

#### Scenario: Folding from a hunk's end keeps what follows in place

- **WHEN** the reader activates the end row of a 120-line hunk while the next hunk's heading sits 200 pixels below the top of the view, and the hunk folds
- **THEN** the next hunk's heading still sits 200 pixels below the top of the view
- **AND** the folded hunk's row sits directly above it

#### Scenario: Showing keeps the heading in place

- **WHEN** a folded hunk's heading is the topmost visible row and the reader activates its "Show" control
- **THEN** that heading is still the topmost visible row, with the hunk's lines below it

#### Scenario: A fold from elsewhere keeps the reader's line

- **WHEN** new line 300 of a file is the topmost visible line and the host folds a hunk above it, without the reader activating anything
- **THEN** new line 300 is still the topmost visible line

#### Scenario: A fold over the reader's line puts its heading on top

- **WHEN** new line 150 is the topmost visible line and the host folds the hunk that holds it, without the reader activating anything
- **THEN** that hunk's folded row is the topmost visible row

#### Scenario: The show control is announced with its state

- **WHEN** assistive technology reaches the control of a folded 12-line hunk whose first line is new line 143
- **THEN** it is a button named "Show 12 lines from new line 143" that is not expanded
