## MODIFIED Requirements

### Requirement: Unified and Side-by-Side Layouts

The diff view SHALL lay out the same model in either of two layouts, unified and side by side, rendering only the layout in effect, each as its own structure in its own document order.

**Unified** SHALL be one column. Each line SHALL show its old and new numbers, its marker and its text, in the model's order. A long line SHALL NOT wrap: a section's lines SHALL scroll sideways inside the section, in a scroller that holds the lines alone.

**Side by side** SHALL render each file's lines as one grid of four columns in two fixed, equal halves, old number and old code on the left and new number and new code on the right, save for a file in one column, as below.

- A context line SHALL fill both halves of one row.
- A **change block** is a maximal run of removed and added lines that no context line interrupts. Its rows SHALL be formed by walking it in order with one open slot, the earliest left-only row of the block not yet given a partner: a removed line opens a left-only row, and an added line fills the open slot's right half, or opens a right-only row when no slot is open. In a block whose removed lines all precede its added lines, as in every block git writes, the i-th removed and the i-th added line therefore share a row, the shorter side's fillers sit at the bottom, and a block of $$k$$ removed and $$m$$ added lines takes $$\max(k, m)$$ rows. Interleaved provider text such as −a +b −c +d pairs a with b and c with d, and an added line never pairs with a removed line that comes after it.
- A line with no partner SHALL face a **filler** cell. A filler SHALL have no number, no marker and no text, SHALL use its own background token, SHALL be hidden from assistive technology, and SHALL never be selected or copied.
- Source lines SHALL NOT wrap. Each code cell SHALL clip its overflow inside its own half and expose that content through horizontal scrolling, retaining all original source whitespace. Horizontal gestures on either code side and a shared, keyboard-accessible scrollbar for the file SHALL move every code viewport in that file to the same horizontal offset. Line numbers, change markers, file header and inline discussion slots SHALL NOT move horizontally. Both halves SHALL remain visible, paired source rows SHALL remain level, and individual lines SHALL NOT display their own scrollbars. The shared scrollbar and a scroll hint SHALL appear only when code overflows its viewport.
- A file whose every line is on one side, with no context line at all (every added or deleted file, and a file emptied or filled from empty), SHALL render in one full-width column headed by that side, rather than beside a column of fillers.
- A file with any context line SHALL keep both columns, even if its hunks only add or only remove, so its context lines face each other with both numbers.

In **both layouts**:

- the hunk header, `@@ -a,b +c,d @@` with its section heading, SHALL be one full-width row;
- a withheld, too-large, binary or hunk-less file SHALL show the same full-width state row, with the same loader where one is offered, and a withheld file loaded while side by side is in effect SHALL render side by side. An image file SHALL instead show its versions, side by side in the left and right halves or stacked in unified, as the *Image Comparison* requirement says;
- the no-newline flag SHALL show as a small badge in each cell that shows the line it marks, so side by side a flagged context line carries it on both sides;
- the sticky file header SHALL be the section's own, above its rows: never a cell of the side-by-side grid, and outside unified's sideways scroller;
- tabs SHALL render at one width;
- markers SHALL stay visible.

The layout SHALL never reach the service: the budgets, the payload, the per-file read and an image file's read SHALL be the same in both layouts.

#### Scenario: A git change block pairs by position

- **WHEN** side by side is in effect and a hunk holds a context line, three removed lines, two added lines and a context line
- **THEN** each context line fills one row on both sides, with its old and new numbers
- **AND** the first and second removed lines face the first and second added lines, and the third removed line faces a filler, in three rows

#### Scenario: Interleaved provider text pairs each added line with the open slot

- **WHEN** side by side is in effect and a change block from provider text is −a +b −c +d
- **THEN** a faces b and c faces d, in two rows

#### Scenario: An added line never pairs with a later removed line

- **WHEN** side by side is in effect and a change block is +a −b
- **THEN** a faces a filler in a right-only row, above a left-only row in which b faces a filler

#### Scenario: A filler carries nothing

- **WHEN** side by side shows a filler facing an unpaired removed line
- **THEN** the filler shows no number, no marker and no text, on its own background
- **AND** it is hidden from assistive technology and is in no selection or copy

#### Scenario: An added file uses one column

- **WHEN** side by side is in effect and a file is added with twenty lines
- **THEN** it renders in one full-width column of the new side, with no fillers

#### Scenario: A file with context keeps both columns

- **WHEN** side by side is in effect and a file's only hunk adds two lines between context lines
- **THEN** it renders in two columns, its context lines facing each other with both numbers and its added lines facing fillers

#### Scenario: Long source lines scroll without wrapping in either layout

- **WHEN** a hunk pairs a 400-character removed line with a short added line
- **THEN** side by side, both source lines occupy a single visual row, the long line is horizontally scrollable within its own half, and the added line starts level with it
- **AND** in unified, the line does not wrap, and the section's lines scroll sideways while its header stays in place

#### Scenario: Deeply indented source stays on one visual row

- **WHEN** a deeply indented TSX line includes ordinary words that fit individually within their side-by-side cell
- **THEN** the complete source line stays on one visual row instead of wrapping continuations back to the left edge
- **AND** the source indentation and paired row alignment remain intact

#### Scenario: Oversized unbroken text stays inside its half

- **WHEN** a split cell contains an identifier or punctuation-only run wider than that cell
- **THEN** its overflow remains clipped to its own half and can be read by horizontal scrolling, without overlapping facing code or widening the table

#### Scenario: Horizontal code movement preserves review context

- **WHEN** the reviewer horizontally scrolls either side of a split file
- **THEN** both code columns and the shared scrollbar follow the same offset without changing the vertical reading position
- **AND** line numbers, change markers, sticky file heading and inline discussion slots remain stationary

#### Scenario: Returning to a retained split view preserves its horizontal position

- **WHEN** the reviewer switches away from a horizontally scrolled Files view and returns
- **THEN** its prior horizontal code position is restored without measuring the hidden panel as a zero-width diff

#### Scenario: Hunk headers and state rows span the full width

- **WHEN** side by side is in effect
- **THEN** each hunk header, `@@ -a,b +c,d @@` with its section heading, is one row across both halves
- **AND** the state row of a binary file that is not an image file spans its section's full width

#### Scenario: A flagged context line shows its badge on both sides

- **WHEN** a context line carries the no-newline flag
- **THEN** side by side, both of its cells show the badge
- **AND** in unified, its one cell shows it

#### Scenario: A withheld file loaded side by side renders side by side

- **WHEN** side by side is in effect and the reader loads a withheld file
- **THEN** its hunks render side by side
