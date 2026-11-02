## ADDED Requirements

### Requirement: Provider-Neutral Diff Line Slots

A diff host SHALL be able to provide optional content for a rendered line by the provider-neutral key of file identity, old/new side and line number. The file identity SHALL be the same key the diff view already uses for the file: its new path, or its old path when deleted. The diff view SHALL resolve that key through the side-qualified identities it already assigns, without receiving a provider thread, comment or anchor type. A renamed file therefore keeps one file identity while its old and new line sides remain distinct.

In unified layout, a populated line slot SHALL render immediately after its line across the section's available width. In side-by-side layout, an old slot SHALL render beneath the old half of its row and a new slot beneath the new half; both SHALL be able to render beneath one paired row. In a one-sided full-width file, that side's slot SHALL use the full width. A context line SHALL accept independent old and new slots because it carries both identities. A filler SHALL never accept a slot.

Line-slot content SHALL be outside the parsed diff model and the model-based selection/copy result. It SHALL not change line numbering, pairing, syntax tokens, hidden-character handling or the text copied from the diff. Each populated slot SHALL have an accessible association naming its side-qualified line. Switching layout, folding or unfolding a hunk, and collapsing or expanding a file SHALL invoke no host action, provider read or re-tokenisation; the same slot content SHALL follow its line when shown.

A key whose file, side or line is not rendered SHALL produce no line slot. The host SHALL decide whether to present that content elsewhere. A host that supplies no line slots, including commit detail, SHALL render exactly as before.

#### Scenario: Unified content follows one line

- **WHEN** a host supplies content for new line 27 of `src/api.ts` in unified layout
- **THEN** the content renders immediately after new line 27 across the section's available width
- **AND** it changes neither that line's number nor copied text

#### Scenario: Both sides of a paired row have slots

- **WHEN** a side-by-side row pairs old line 12 with new line 14 and the host supplies content for both identities
- **THEN** the old content renders beneath the old half and the new content beneath the new half
- **AND** neither is attached to the other side or to a filler

#### Scenario: A context line carries two independent anchors

- **WHEN** a context line is old line 20 and new line 23 and the host supplies only an old-side slot
- **THEN** the old-side content renders in both layouts
- **AND** no new-side content is invented

#### Scenario: Switching layout preserves line content without work

- **WHEN** line slots are populated and the reader switches between unified and side by side
- **THEN** each slot follows the same side-qualified line identity
- **AND** no command, host action, provider read or syntax-tokenisation runs

#### Scenario: A missing line renders no slot

- **WHEN** a host supplies content for a line absent from the rendered model
- **THEN** the diff view renders no slot for it
- **AND** it does not move the content to a numerically similar line

### Requirement: Line Actions in the Gutter

A diff host SHALL be able to provide one optional action for a rendered line, by the same side-qualified identity as a line slot. The diff view SHALL draw it in that line's number gutter, at the edge beside the code, and SHALL show it only while the line's row (side by side, that half of the row) is hovered or the action holds keyboard focus. A line action SHALL take no row and no column of its own, and SHALL change no line number, pairing, syntax token or copied text.

In unified layout a row SHALL show at most one action: the new side's when the host gives one, otherwise the old side's, so a context line offers one action on its one row. Side by side, each half SHALL show its own side's action. A filler SHALL never show an action. A host that supplies no line actions SHALL render exactly as before.

#### Scenario: A line offers its action on hover without a row

- **WHEN** a host supplies an action for every rendered line of `src/api.ts`
- **THEN** each line shows its action in its gutter while hovered or focused
- **AND** no line slot row is added for any line

#### Scenario: A unified context line offers one action

- **WHEN** a unified context line is old line 20 and new line 20, and the host supplies an action for both identities
- **THEN** the row shows only the new side's action
- **AND** it shows the old side's action when the host supplies none for the new side

## MODIFIED Requirements

### Requirement: Diff View Hosts

The diff view SHALL be the one view through which every host shows a diff, such as commit detail or a pull request's view, so no host forks the renderer. A host SHALL pass it the model, the names of the two sides and a loader. Optional slots SHALL carry everything else a host adds: two per file, two per hunk and provider-neutral old/new line slots:

- the **header extra**, rendered inside the file's sticky header, where a pull request's "viewed" mark sits;
- the **preamble**, rendered between the header and the first hunk, across the section's full width, where unplaced or ranged pull-request content sits;
- the **hunk heading extra**, rendered in a hunk's heading row, in the gutter left of its `@@` heading, where a pull request's hunk mark sits;
- the **hunk end**, rendered as a row after a hunk's last line, across the section's full width, only when the host gives it content for that hunk;
- the **line slot**, keyed by the file identity, old/new side and line number and rendered as the *Provider-Neutral Diff Line Slots* requirement says, where a pull request's current single-line content sits.

No file or hunk slot SHALL be given a diff column, and each SHALL render the same in either layout. A line slot SHALL follow its named side as *Provider-Neutral Diff Line Slots* requires. A host SHALL be permitted to name the hunks it folds (see the *Folded Hunks* requirement) and to ask the view to collapse a file's section once (see the *File Sections* requirement). A host that passes no hunk or line slots and folds nothing SHALL see its hunks exactly as before: commit detail passes none.

**Image reader.** A host SHALL be permitted to pass an **image reader**, which reads one image file's two versions (see the *Image Comparison* requirement). It SHALL say when those reads happen: **as a section nears the view**, or **on request**. Commit detail SHALL read as a section nears the view, and a pull request's view SHALL read on request. A host that passes no image reader SHALL see image files as any other files.

The loader SHALL be offered only for a withheld file. A too-large file, and a binary file that is not an image file, SHALL show its state and no control. An image file SHALL offer only what the *Image Comparison* requirement gives it.

The view SHALL show the two side names in its toolbar while side by side is in effect, with the escapes the *Hidden Characters Are Shown* requirement gives side names. Commit detail SHALL name the old side by the first parent's abbreviated id, or "empty tree" for a root commit, and the new side by the commit's abbreviated id.

#### Scenario: Slots render the same in both layouts

- **WHEN** a host fills a file's header extra with a mark and its preamble with a note, and the reader switches between the layouts
- **THEN** in both layouts the mark sits in the file's sticky header and the note spans the section's full width between the header and the first hunk

#### Scenario: Only a withheld file offers the loader

- **WHEN** a diff holds a withheld, a too-large and a binary file, none of them an image file
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
- **THEN** no hunk carries a heading extra or an end row, no line carries a host slot, and no hunk is folded

#### Scenario: Each host reads its images when it says

- **WHEN** commit detail and a pull request's view each show a modified image file
- **THEN** commit detail reads it once its section nears the view, with no control
- **AND** the pull request's view shows "Show image" and reads it only when the reader activates that control

#### Scenario: Image files read only through the image reader

- **WHEN** a host shows an image file
- **THEN** its versions are read through that host's image reader, and its loader is never offered for it

### Requirement: Host Slot Updates Re-render Only Their File

A host's slots and folds for one file SHALL keep their identity for as long as that file's own inputs are unchanged (see the *Diff View Hosts* requirement). This covers the header extra, the preamble, the hunk heading extra, hunk end, provider-neutral line slots and folded hunks. When a host's state changes, the view SHALL render again only the sections of files whose slots or folds changed. Every other section SHALL keep its rows' elements and SHALL NOT render again. This SHALL hold for files shown within the eager budget and for files loaded on request alike (see the *Line and Byte Budgets With On-Request Loading* requirement).

A host change that alters no file's slots or folds SHALL render no section again. It SHALL NOT re-run the view's fold bookkeeping either: the pruning of shown hunks and the reader's-place anchor.

This SHALL NOT weaken the guarantees of the *Folded Hunks* requirement. A change to one file's folds SHALL still drop the shown state of every hunk of that file whose fold changed. It SHALL still keep the reader's place by the table that requirement gives. Slots SHALL still render the same in either layout. A host that passes no slots, such as commit detail, SHALL see no change in what renders.

#### Scenario: A change to one file's slots renders that file alone

- **WHEN** a host shows 40 files and changes the header extra or one line slot of the 7th file only
- **THEN** the 7th file's section renders with its new slot content
- **AND** the other 39 sections keep every row's element and do not render again

#### Scenario: A host change that touches no slot renders no section

- **WHEN** a host re-renders the view for a reason that changes no file's slots or folds
- **THEN** no file section renders again
- **AND** the shown hunks and the reader's place are left exactly as they were

#### Scenario: A refold still drops the shown state of that file

- **WHEN** the reader showed a folded hunk of one file, the host stopped folding it, and the host then folds it again while every other file's slots stay unchanged
- **THEN** that hunk is folded
- **AND** no other file's section renders again

#### Scenario: A file loaded on request takes part

- **WHEN** a withheld file was loaded on request and the host then changes another file's slots
- **THEN** the loaded file's section does not render again
