## MODIFIED Requirements

### Requirement: Line and Byte Budgets With On-Request Loading

Which files arrive with their hunks SHALL be decided by line and byte **budgets**, so the lines on the page stay bounded however large the diff, in either layout. The budgets SHALL decide only among **patched** files, those with a patch to show, taken in the model's file order. A binary file, and a file too large because it lies past a read ceiling that leaves it unreadable, SHALL keep its own state, SHALL add nothing to the line total, and SHALL never be withheld. A file whose provider omitted its patch for size, while reporting changed lines, SHALL instead arrive **withheld** whatever the budgets decide: its host reads it on request (see the *GitHub Detail Reads* requirement in the `pull-request-viewer` capability). It SHALL add nothing to the line total either.

With $$c(f)$$ the added plus removed lines of file $$f$$, context lines not counted, and $$g \prec f$$ when $$g$$ precedes $$f$$ in the model's file order, a file SHALL arrive with its hunks exactly when:

$$\text{eager}(f) \iff \text{patched}(f) \;\wedge\; c(f) \le 500 \;\wedge\; c(f) + \sum_{g \prec f,\ \text{eager}(g)} c(g) \le 3000$$

Every other patched file SHALL be withheld. Lines are not bytes, so two byte limits SHALL also apply as the patch text is read:

- a file SHALL be withheld once its own patch text passes 64 KiB;
- once the eager files' patch text reaches 1 MiB in total, every remaining file SHALL be withheld and reading SHALL stop. The file whose text reaches it keeps its hunks, so the eager text stays under 1 MiB plus one file's 64 KiB, and no patch text is read only to be discarded.

A file a byte limit withholds SHALL keep its lines in the line total, so the byte limits only ever shrink the eager set the line rule decided. A commit's streamed read SHALL give up once it has read 8 MiB of patch text in all, withholding every remaining patched file, each of which can still be read on its own (see the *Commit Detail View* requirement in the `commit-graph` capability). A host whose read ceiling leaves the files past it unreadable SHALL instead make those files too large to preview before the budgets apply. None of these limits SHALL be a setting, and none SHALL depend on the layout: the budgets are decided before any layout, and both layouts withhold the same files.

A withheld file SHALL reach the frontend with its counts and without its patch. It SHALL render collapsed, with its counts, and with a "Load diff" control in place of its lines. Activating the control SHALL read that file alone, through the host's loader, under a per-file ceiling of 8 MiB of diff text: the file SHALL then show its hunks or, past the ceiling, become too large to preview. A load that cannot complete SHALL leave the file withheld, show the loader's reason beside the control, and leave the control to try again. A too-large file SHALL read "too large to preview", and neither it nor a binary file SHALL offer a control to load it. A host MAY give a too-large file a way to view it elsewhere, in its per-file slots.

The budgets bound lines, not files: every file, whatever its content state, SHALL keep its navigator row with its counts and its section header. The view SHALL state how many files are not shown in full.

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
