## MODIFIED Requirements

### Requirement: Line and Byte Budgets With On-Request Loading

Which files arrive with their hunks SHALL be decided by line and byte **budgets**, so the lines on the initial page stay bounded however large the diff, in either layout. The budgets SHALL decide only among **patched** files, those with a patch to show, taken in the model's file order. A binary file, and a file too large because it lies past a read ceiling that leaves it unreadable, SHALL keep its own state, SHALL add nothing to the line total, and SHALL never be withheld. A file whose provider omitted its patch for size, while reporting changed lines, SHALL instead arrive **withheld** whatever the budgets decide: its host reads it through the per-file loader (see the *GitHub Detail Reads* requirement in the `pull-request-viewer` capability). It SHALL add nothing to the line total either.

With $$c(f)$$ the added plus removed lines of file $$f$$, context lines not counted, and $$g \prec f$$ when $$g$$ precedes $$f$$ in the model's file order, a file SHALL arrive with its hunks exactly when:

$$\text{eager}(f) \iff \text{patched}(f) \;\wedge\; c(f) \le 500 \;\wedge\; c(f) + \sum_{g \prec f,\ \text{eager}(g)} c(g) \le 3000$$

Every other patched file SHALL be withheld. Lines are not bytes, so two byte limits SHALL also apply as the patch text is read:

- a file SHALL be withheld once its own patch text passes 64 KiB;
- once the eager files' patch text reaches 1 MiB in total, every remaining file SHALL be withheld and reading SHALL stop. The file whose text reaches it keeps its hunks, so the eager text stays under 1 MiB plus one file's 64 KiB, and no patch text is read only to be discarded.

A file a byte limit withholds SHALL keep its lines in the line total, so the byte limits only ever shrink the eager set the line rule decided. A commit's streamed read SHALL give up once it has read 8 MiB of patch text in all, withholding every remaining patched file, each of which can still be read on its own (see the *Commit Detail View* requirement in the `commit-graph` capability). A host whose read ceiling leaves the files past it unreadable SHALL instead make those files too large to preview before the budgets apply. None of these limits SHALL be a setting, and none SHALL depend on the layout: the budgets are decided before any layout, and both layouts withhold the same files.

A withheld file SHALL reach the frontend with its counts and without its patch. It SHALL render collapsed, with its counts, and with a "Load diff" control in place of its lines. Activating the control SHALL read that file alone, through the host's loader, under a per-file ceiling of 8 MiB of diff text: the file SHALL then show its hunks or, past the ceiling, become too large to preview. A load that cannot complete SHALL leave the file withheld, show the loader's reason beside the control, and leave the control to try again. A too-large file SHALL read "too large to preview", and neither it nor a binary file SHALL offer a control to load its diff. An image file instead offers its two versions, read apart from its diff and counted by no budget (see the *Image Comparison* requirement). A host MAY give a too-large file, or an image file whose versions cannot all be shown, a way to view it elsewhere, in its per-file slots.

A host SHALL additionally be permitted to request automatic withheld-file loading as content nears the view, as the *Diff View Hosts* requirement specifies. That SHALL invoke the same per-file loader, not raise any eager budget or hard ceiling, and SHALL leave the manual control and failure/retry behaviour available. With no host opt-in, loading SHALL remain on request. Automatic insertion SHALL preserve reading position and focus, without expanding a reader-collapsed section or remounting other files/editors.

The budgets bound initial lines, not files: every file, whatever its content state, SHALL keep its navigator row with its counts and its section header. The view SHALL state how many files are not shown in full, updating as loads settle.

#### Scenario: Files are eager in order while the total allows

- **WHEN** a diff's patched files, in order, change 500, 480, 2,000, 490, 490, 490, 490, 100 and 60 lines
- **THEN** the third file is withheld, being over 500 lines
- **AND** the eighth is withheld, since it would take the eager total to 3,040
- **AND** every other file arrives with its hunks, the ninth taking the eager total to exactly 3,000

#### Scenario: Context lines are not counted

- **WHEN** a diff's first patched file changes 500 lines and its hunks also hold 60 context lines
- **THEN** it arrives with its hunks

#### Scenario: Binary and too-large files keep their own state

- **WHEN** a diff whose patched files exhaust the line total also lists a binary file and a file past its host's read ceiling
- **THEN** the binary file stays binary and the other stays too large to preview
- **AND** neither adds to the line total, and neither offers "Load diff"

#### Scenario: A file whose provider omitted its patch is withheld

- **WHEN** a pull request's files list one with 1,635 changed lines whose provider omitted its patch for size
- **THEN** that file arrives withheld, with its counts and a "Load diff" control
- **AND** it adds nothing to the line total

#### Scenario: A byte limit only shrinks the eager set

- **WHEN** a diff's patched files, in order, change 300, 500, 500, 500, 500, 500, 500 and 200 lines, and the first file's patch text is 70 KiB
- **THEN** the first file is withheld by the per-file byte limit
- **AND** the seventh file is still withheld and the eighth still arrives with its hunks, as the line rule decided

#### Scenario: The eager files' text stops at 1 MiB

- **WHEN** eager files whose patch text totals 1 MiB have been read, and later files are eager by the line rule
- **THEN** those later files are withheld
- **AND** no further patch text is read

#### Scenario: A read gives up at 8 MiB

- **WHEN** a streamed read of a commit's patch text has read 8 MiB in all before reaching its remaining eager files
- **THEN** the read gives up, and every remaining patched file is withheld, each still loadable on request

#### Scenario: A withheld file loads on request

- **WHEN** the reader activates "Load diff" on a withheld file
- **THEN** that file alone is read, and its section shows its hunks
- **AND** no other file's content changes

#### Scenario: A load that cannot complete can be tried again

- **WHEN** the host's loader rejects a withheld file's load with a reason
- **THEN** the file stays withheld, the reason is shown beside "Load diff", and "Load diff" still works

#### Scenario: A file past the per-file ceiling is too large to preview

- **WHEN** the reader loads a withheld file whose diff text exceeds 8 MiB
- **THEN** its section reads "too large to preview" and offers no control to load it

#### Scenario: Every file keeps its row and its header

- **WHEN** a diff of 900 files has most of them withheld
- **THEN** each of the 900 files has a navigator row with its counts and a section header

#### Scenario: The view counts the files it does not show in full

- **WHEN** three of a diff's files are withheld and every other file arrives with its hunks
- **THEN** the view states that three files are not shown in full

#### Scenario: Both layouts withhold the same files

- **WHEN** the same diff is opened once with unified in effect and once with side by side in effect
- **THEN** the same files are withheld in both

#### Scenario: Automatic loads do not change the initial model

- **WHEN** a PR host enables automatic withheld-file loading
- **THEN** the initial model is still budgeted identically to the model with automation off
- **AND** only later eligible per-file loads add hunks to the view

#### Scenario: A background load preserves the reader's place

- **WHEN** a host-enabled automatic load inserts hunks above the reader's current line
- **THEN** the current content anchor and focus remain in place
- **AND** no unrelated file or active editor is remounted

### Requirement: Diff View Hosts

The diff view SHALL be the one view through which every host shows a diff, such as commit detail or a pull request's view, so no host forks the renderer. A host SHALL pass it the model, the names of the two sides and a loader. Optional slots SHALL carry everything else a host adds: two per file, two per hunk and provider-neutral old/new line slots:

- the **header extra**, rendered inside the file's sticky header, where a pull request's "viewed" mark sits;
- the **preamble**, rendered between the header and the first hunk, across the section's full width, where unplaced or ranged pull-request content sits;
- the **hunk heading extra**, rendered in a hunk's heading row, in the gutter left of its `@@` heading, where a pull request's hunk mark sits;
- the **hunk end**, rendered as a row after a hunk's last line, across the section's full width, only when the host gives it content for that hunk;
- the **line slot**, keyed by the file identity, old/new side and line number and rendered as the *Provider-Neutral Diff Line Slots* requirement says, where a pull request's current single-line content sits.

No file or hunk slot SHALL be given a diff column, and each SHALL render the same in either layout. A line slot SHALL follow its named side as *Provider-Neutral Diff Line Slots* requires. A host SHALL be permitted to name the hunks it folds (see the *Folded Hunks* requirement) and to ask the view to collapse a file's section once (see the *File Sections* requirement). A host that passes no hunk or line slots and folds nothing SHALL see its hunks exactly as before: commit detail passes none.

**Image reader.** A host SHALL be permitted to pass an **image reader**, which reads one image file's two versions (see the *Image Comparison* requirement). It SHALL say when those reads happen: **as a section nears the view**, or **on request**. Commit detail SHALL read as a section nears the view. A pull request's view SHALL select near-view or on-request reads according to its image preference (see the *Pull-Request Loading Preferences* requirement in the `pr-content-loading` capability). A host that passes no image reader SHALL see image files as any other files.

**Withheld-file reader.** A host SHALL be permitted to select near-view automatic loading through its existing loader and coordinate automatic work with other content in its presentation. With no selection it SHALL remain on request, and commit detail SHALL retain its existing manual withheld-file loading. A PR host SHALL follow the *Automatic Loads Follow Visible Content* requirement in the `pr-content-loading` capability. The view SHALL expose visibility/collapse and model lifetime to that policy without forking its renderer or changing file-slot identity.

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

- **WHEN** commit detail and a pull request's view each show a modified image file and the PR image preference is off
- **THEN** commit detail reads it once its section nears the view, with no control
- **AND** the pull request's view shows "Show image" and reads it only when the reader activates that control

#### Scenario: Image files read only through the image reader

- **WHEN** a host shows an image file
- **THEN** its versions are read through that host's image reader, and its loader is never offered for it

#### Scenario: PR automation does not change commit loading

- **WHEN** a PR host enables automatic images and withheld-file loading
- **THEN** commit detail still reads images near the view and withheld diffs only on request
- **AND** both hosts continue to use the same renderer
