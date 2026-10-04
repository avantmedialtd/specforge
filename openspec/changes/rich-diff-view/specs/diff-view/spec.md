## ADDED Requirements

### Requirement: Diff Model

The diff view SHALL render a parsed **diff model** and SHALL parse no diff text itself. The model SHALL be produced by pure functions in `openspec-core`, with one meaning whatever its source:

- a full unified diff, as git writes it and as BitBucket serves it, SHALL parse to a list of files;
- a header-less per-file patch that starts at its first hunk header, such as GitHub's `patch` field, SHALL parse to that file's hunks alone, and the file's status, paths and counts SHALL then come from the provider's own fields.

Each **file** SHALL carry:

- an old path, absent when the file was added, and a new path, absent when it was deleted;
- an old mode and a new mode, each git's octal mode and each absent when the source gives none;
- a status: added, modified, deleted, renamed or copied (each with its similarity when the source gives one), mode-changed, or type-changed;
- its counts of added and removed lines, each absent when the source gives none;
- its content, in exactly one of four states: its **hunks** (none when the file has no textual change); **withheld** by the budgets and loadable on request; **too large** to preview; or **binary** (see the *Line and Byte Budgets With On-Request Loading* requirement).

Where the source gives both modes, mode-changed SHALL be the status of a change to the mode alone, and a file whose content and mode both changed SHALL be modified, with both modes carried. A source that gives no modes, such as GitHub's file list, SHALL keep the status the provider reports. A type change (file ↔ symlink ↔ submodule), which git writes as a section deleting the path followed directly by a section creating it, SHALL fold into one type-changed file carrying both sections' hunks. The fold SHALL apply whenever the two sections' `deleted file mode` and `new file mode` differ in file type, for a commit, for provider text and for a one-file read alike.

Each **hunk** SHALL carry its old start and line count, its new start and line count, its section heading when its header has one, and its lines. Each **line** SHALL be context, added or removed, and SHALL carry its text, an old line number when it is context or removed, and a new line number when it is context or added, assigned from the hunk's ranges. A line that ends its file without a newline SHALL carry a **no-newline flag**: the parser SHALL fold git's `\ No newline at end of file` marker into the line before it, whether that line is removed, added or context, and the marker SHALL never become a line of its own.

The parser SHALL understand git's extended headers (`rename from` and `rename to`, `copy from` and `copy to`, `similarity index`, `new file mode`, `deleted file mode`, `old mode` and `new mode`, `Binary files … differ`, `Subproject commit`) and git's C-quoted paths. It SHALL decode bytes lossily, per line of patch text and per record of a file list, never as one strict string, so text in another encoding shows replacement characters in its own lines or path and no other file is affected.

Paths SHALL never be guessed from an ambiguous header:

- for a commit's detail read, each patch section, after the type-change fold, SHALL take its paths from the file-list record it pairs with in git's output order; a one-file read on request, which has no file list, SHALL be named as provider text is;
- for provider text, paths SHALL come from `rename from` and `rename to`, or `copy from` and `copy to`, else from the `---` and `+++` lines, dropping the tab git appends after a name that contains a space;
- only a section with neither (a mode-only change, an empty added or deleted file, a binary file) SHALL be named from its `diff --git` line, split where its two halves name the same path.

The model SHALL carry no layout and no highlighting: nothing in it, or in the payload that carries it, SHALL depend on the layout in effect. It SHALL cross both transports with camelCase field names. The status and the content SHALL each be a tagged union discriminated by a `kind` field, whose values are `added`, `modified`, `deleted`, `renamed`, `copied`, `modeChanged` and `typeChanged` for the status, and `hunks`, `withheld`, `tooLarge` and `binary` for the content. A line's kind SHALL be the string `context`, `added` or `removed`, and the no-newline flag SHALL be the field `noNewline` of the line it qualifies, omitted when false.

#### Scenario: A rename is one file

- **WHEN** a diff renames `src/old.ts` to `src/new.ts` at 92% similarity and changes one of its lines
- **THEN** the model holds one renamed file with old path `src/old.ts`, new path `src/new.ts` and similarity 92
- **AND** its content is the hunk that changes the line, and no added or deleted file is listed for either path

#### Scenario: A type change is one file

- **WHEN** a commit replaces the regular file `bin/tool` with a symlink of the same name
- **THEN** the model holds one type-changed file for `bin/tool`, carrying the hunks of both the deletion and the creation sections
- **AND** no added or deleted file is listed for `bin/tool`

#### Scenario: A mode-only change has its own status

- **WHEN** a diff changes `run.sh` from mode `100644` to `100755` and changes nothing else
- **THEN** its status is mode-changed, with old mode `100644` and new mode `100755`

#### Scenario: An edited file whose mode changed is modified

- **WHEN** a diff changes `run.sh` from mode `100644` to `100755` and also edits one of its lines
- **THEN** its status is modified, with both modes carried, and its content is the hunk that edits the line

#### Scenario: Line numbers follow the hunk ranges

- **WHEN** a hunk headed `@@ -10,3 +10,4 @@ fn main()` holds a context line, a removed line, two added lines and a context line, in that order
- **THEN** its section heading is `fn main()`
- **AND** its lines are numbered old 10 and new 10, old 11, new 11, new 12, and old 12 and new 13

#### Scenario: The no-newline marker qualifies the line before it

- **WHEN** a change block's last removed line and its last added line are each followed by `\ No newline at end of file`
- **THEN** that removed line and that added line each carry the no-newline flag
- **AND** the hunk holds no line for either marker

#### Scenario: Adding only the final newline flags the old line alone

- **WHEN** a change only adds the newline missing after a file's last line
- **THEN** the hunk holds a removed line that carries the no-newline flag, and an added line with the same text that does not

#### Scenario: A context line can carry the flag

- **WHEN** a file whose last line lacks a newline is changed above that line, and its hunk ends with that line as context followed by the marker
- **THEN** that context line carries the no-newline flag

#### Scenario: A header-less patch parses to hunks

- **WHEN** a GitHub `patch` field beginning `@@ -1,2 +1,3 @@` is parsed
- **THEN** it yields that file's hunks, numbered from their ranges
- **AND** the file's status, paths and counts are the ones the provider gave

#### Scenario: One file in another encoding affects only itself

- **WHEN** a commit changes three files and the second one's changed line is encoded in Latin-1
- **THEN** that line shows replacement characters for the bytes that are not UTF-8
- **AND** the other two files' lines, and all three paths, read exactly as they are

#### Scenario: A non-UTF-8 path affects only itself

- **WHEN** a commit's file list holds one path containing a byte that is not UTF-8
- **THEN** that path shows a replacement character in place of the byte
- **AND** every other path in the list reads exactly as it is

#### Scenario: A C-quoted path is unquoted

- **WHEN** provider text names a file in its `---` and `+++` lines as `"a/caf\303\251.md"` and `"b/caf\303\251.md"`
- **THEN** the file's path is `café.md`

#### Scenario: A name containing a space keeps its exact path

- **WHEN** provider text changes `my notes.md`, whose `---` and `+++` lines end with the tab git appends after a name containing a space
- **THEN** the file's path is `my notes.md`, with no trailing tab

#### Scenario: An ambiguous diff line is split on its matching halves

- **WHEN** provider text holds a mode-only change to the path `x b/y`, named only by the line `diff --git a/x b/y b/x b/y`
- **THEN** the file's path is `x b/y`

#### Scenario: The model crosses the wire with exact discriminants

- **WHEN** a model holding a mode-changed file, a type-changed file, a too-large file and a removed line that carries the no-newline flag is sent over either transport
- **THEN** the two files' status `kind` values are `modeChanged` and `typeChanged`, the too-large file's content `kind` is `tooLarge`, and the line's `kind` is `removed`
- **AND** that line has `noNewline` set to `true`, and no unflagged line has a `noNewline` field

#### Scenario: The model does not depend on the layout

- **WHEN** the same commit is read once while unified is in effect and once while side by side is in effect
- **THEN** both reads return the identical model

### Requirement: Line and Byte Budgets With On-Request Loading

Which files arrive with their hunks SHALL be decided by line and byte **budgets**, so the lines on the page stay bounded however large the diff, in either layout. The budgets SHALL decide only among **patched** files, those with a patch to show, taken in the model's file order. A binary file, and a file already too large (its provider omitted its patch for size, or it lies past a read ceiling that leaves it unreadable), SHALL keep its own state, SHALL add nothing to the line total, and SHALL never be withheld.

With $$c(f)$$ the added plus removed lines of file $$f$$, context lines not counted, and $$g \prec f$$ when $$g$$ precedes $$f$$ in the model's file order, a file SHALL arrive with its hunks exactly when:

$$\text{eager}(f) \iff \text{patched}(f) \;\wedge\; c(f) \le 500 \;\wedge\; c(f) + \sum_{g \prec f,\ \text{eager}(g)} c(g) \le 3000$$

Every other patched file SHALL be withheld. Lines are not bytes, so two byte limits SHALL also apply as the patch text is read:

- a file SHALL be withheld once its own patch text passes 64 KiB;
- once the eager files' patch text reaches 1 MiB in total, every remaining file SHALL be withheld and reading SHALL stop.

A file a byte limit withholds SHALL keep its lines in the line total, so the byte limits only ever shrink the eager set the line rule decided. A commit's streamed read SHALL give up once it has read 8 MiB of patch text in all, withholding every remaining patched file, each of which can still be read on its own (see the *Commit Detail View* requirement in the `commit-graph` capability). A host whose read ceiling leaves the files past it unreadable SHALL instead make those files too large to preview before the budgets apply. None of these limits SHALL be a setting, and none SHALL depend on the layout: the budgets are decided before any layout, and both layouts withhold the same files.

A withheld file SHALL reach the frontend with its counts and without its patch. It SHALL render collapsed, with its counts, and with a "Load diff" control in place of its lines. Activating the control SHALL read that file alone, through the host's loader, under a per-file ceiling of 8 MiB of diff text: the file SHALL then show its hunks or, past the ceiling, become too large to preview. A too-large file SHALL read "too large to preview", and neither it nor a binary file SHALL offer a control to load it.

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

- **WHEN** a diff whose patched files exhaust the line total also lists a binary file and a file whose provider omitted its patch for size
- **THEN** the binary file stays binary and the other stays too large to preview
- **AND** neither adds to the line total, and neither offers "Load diff"

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

### Requirement: File Navigator

The diff view SHALL present a **navigator**: a collapsible tree of the changed files by directory, compacting single-child directories into one row, that lists every file of the model whatever its content state, each with its status and with its counts where the source gives them. Activating a file SHALL scroll its section into view, and as the reader scrolls, the navigator SHALL mark the file whose section is being read.

The navigator SHALL be operable from the keyboard as the workspace tree is (see the *Workspace Tree Keyboard Navigation* requirement in the `spec-browser` capability): it SHALL occupy one position in the Tab order with a roving current row, the arrow keys SHALL move between its visible rows and open and close its directories, and Enter or Space on a file SHALL activate it exactly as a click does.

Where the diff view is too narrow for the navigator to fit beside the file sections, the navigator SHALL fold into a list above them. That SHALL be decided by the diff view's own width, never by the window's, as Settings decides its group navigation (see the *Group Navigation Follows the View's Own Width* requirement in the `settings-view` capability). While side by side is chosen, the navigator SHALL also fold where the *Narrow Views Fall Back to Unified* requirement says.

#### Scenario: Files are grouped by directory

- **WHEN** a diff changes `src/components/diff/DiffView.tsx`, `src/components/diff/rows.ts` and `README.md`
- **THEN** the navigator shows one directory row `src/components/diff` holding the two files, and `README.md` at the top level
- **AND** each file row shows its status and its added and removed counts

#### Scenario: Activating a file scrolls to it

- **WHEN** the reader activates `rows.ts` in the navigator
- **THEN** its section scrolls into view
- **AND** the navigator marks `rows.ts` as the file being read

#### Scenario: Scrolling moves the mark

- **WHEN** the reader scrolls from the first file's section into the second's
- **THEN** the navigator's mark moves from the first file to the second

#### Scenario: The navigator is one Tab stop

- **WHEN** the reader tabs into the navigator, moves to a file row with the arrow keys and presses Enter
- **THEN** Tab focused the navigator's current row alone, not each of its rows
- **AND** the file's section scrolls into view, as a click on its row would

#### Scenario: A narrow view folds the navigator above the sections

- **WHEN** the diff view is too narrow for the navigator to fit beside the file sections
- **THEN** the navigator is a list above the sections

#### Scenario: The window's width does not fold the navigator

- **WHEN** the window is wide but the side panes leave the diff view too narrow for the navigator beside the sections
- **THEN** the navigator folds above the sections

### Requirement: File Sections

Each file SHALL render as a **section**: a header, then the file's content. The header SHALL stick to the top of the view while the reader scrolls through its section, and SHALL show:

- the file's path, or, for a renamed file, its old path, an arrow and its new path (`old → new`);
- its status;
- its old and new modes, whenever they differ;
- its counts, where the source gives them;
- a toggle that collapses and expands the section.

A renamed file whose content also changed SHALL show its hunks below that header.

Every line SHALL show its line numbers in a gutter (in unified, its old and new numbers side by side; side by side, one number beside each column), and every added or removed line its marker, `+` or `−`, in both layouts, so additions and removals are never conveyed by colour, or by column, alone. Added and removed lines SHALL also carry a background tint (see the *Syntax Highlighting* requirement).

Collapse SHALL be view state held by the diff view and kept per file. It SHALL NOT be a preference and SHALL NOT be persisted.

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

### Requirement: Unified and Side-by-Side Layouts

The diff view SHALL lay out the same model in either of two layouts, unified and side by side, rendering only the layout in effect, each as its own structure in its own document order.

**Unified** SHALL be one column. Each line SHALL show its old and new numbers, its marker and its text, in the model's order. A long line SHALL NOT wrap: a section's lines SHALL scroll sideways inside the section, in a scroller that holds the lines alone.

**Side by side** SHALL render each file's lines as one grid of four columns in two fixed, equal halves, old number and old code on the left and new number and new code on the right, save for a file in one column, as below.

- A context line SHALL fill both halves of one row.
- A **change block** is a maximal run of removed and added lines that no context line interrupts. Its rows SHALL be formed by walking it in order with one open slot, the earliest left-only row of the block not yet given a partner: a removed line opens a left-only row, and an added line fills the open slot's right half, or opens a right-only row when no slot is open. In a block whose removed lines all precede its added lines, as in every block git writes, the i-th removed and the i-th added line therefore share a row, the shorter side's fillers sit at the bottom, and a block of $$k$$ removed and $$m$$ added lines takes $$\max(k, m)$$ rows. Interleaved provider text such as −a +b −c +d pairs a with b and c with d, and an added line never pairs with a removed line that comes after it.
- A line with no partner SHALL face a **filler** cell. A filler SHALL have no number, no marker and no text, SHALL use its own background token, SHALL be hidden from assistive technology, and SHALL never be selected or copied.
- A long line SHALL wrap inside its cell, breaking anywhere, with no line number on its continuation. A row SHALL take the height of its taller cell, so paired lines stay level.
- A file whose every line is on one side, with no context line at all (every added or deleted file, and a file emptied or filled from empty), SHALL render in one full-width column headed by that side, rather than beside a column of fillers.
- A file with any context line SHALL keep both columns, even if its hunks only add or only remove, so its context lines face each other with both numbers.

In **both layouts**:

- the hunk header, `@@ -a,b +c,d @@` with its section heading, SHALL be one full-width row;
- a withheld, too-large, binary or hunk-less file SHALL show the same full-width state row, with the same loader where one is offered, and a withheld file loaded while side by side is in effect SHALL render side by side;
- the no-newline flag SHALL show as a small badge in each cell that shows the line it marks, so side by side a flagged context line carries it on both sides;
- the sticky file header SHALL be the section's own, above its rows: never a cell of the side-by-side grid, and outside unified's sideways scroller;
- tabs SHALL render at one width;
- markers SHALL stay visible.

The layout SHALL never reach the service: the budgets, the payload and the per-file read SHALL be the same in both layouts.

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

#### Scenario: Long lines wrap side by side and scroll in unified

- **WHEN** a hunk pairs a 400-character removed line with a short added line
- **THEN** side by side, the removed line wraps inside its cell with no number on its continuation, the row is as tall as the wrapped cell, and the added line starts level with it
- **AND** in unified, the line does not wrap, and the section's lines scroll sideways while its header stays in place

#### Scenario: Hunk headers and state rows span the full width

- **WHEN** side by side is in effect
- **THEN** each hunk header, `@@ -a,b +c,d @@` with its section heading, is one row across both halves
- **AND** a binary file's state row spans its section's full width

#### Scenario: A flagged context line shows its badge on both sides

- **WHEN** a context line carries the no-newline flag
- **THEN** side by side, both of its cells show the badge
- **AND** in unified, its one cell shows it

#### Scenario: A withheld file loaded side by side renders side by side

- **WHEN** side by side is in effect and the reader loads a withheld file
- **THEN** its hunks render side by side

### Requirement: The Layout Choice Is Per Surface

The diff view SHALL offer the choice between its layouts as a two-option radio group, labelled "Unified" and "Side by side", in the view's toolbar. The view SHALL own the control, so every host offers the same choice. The control SHALL have text labels and SHALL always be visible, never revealed on hover (see the *Essential Controls Are Discoverable Without Hover* requirement in the `touch-input` capability). No keyboard shortcut, menu item or Settings row SHALL choose the layout.

The choice SHALL be per-surface view state and never an application setting. It SHALL be stored under `specforge.diffLayout` in the surface's own `localStorage`: every desktop window shares one origin and so one choice, each browser that opens a served instance keeps its own, and a choice made on one surface never changes another's. Side by side SHALL be stored as exactly `split`; any other stored value, and no value, SHALL read as unified, the default.

A switch SHALL apply at once to the view where it is made, and SHALL be stored. Every diff the surface opens afterwards (another commit, or a pull request opened or reloaded, in the center pane or in a window of its own) SHALL start in the stored layout. A view already open SHALL keep its layout until the reader switches it there or it opens another commit or pull request, and a re-read of the same pull request SHALL keep it; a switch in one window therefore SHALL NOT re-lay out a view open in another.

Reads and writes of the stored choice SHALL be best-effort: a read that fails SHALL read as unified, and a write that fails SHALL keep the choice for the view where it was made only.

The layout SHALL NOT be part of the Address or of any window's URL (see the *Addressable Viewing State* requirement in the `view-routing` capability).

#### Scenario: Unified is the default

- **WHEN** a surface that has never stored a layout opens a commit's diff
- **THEN** the diff renders unified, with "Unified" checked

#### Scenario: A switch applies at once and is remembered

- **WHEN** the reader chooses "Side by side" in commit detail, in a view whose sections column is 120 ch wide, and then opens another commit
- **THEN** the first commit's diff re-lays out side by side at once
- **AND** the second commit's diff opens side by side
- **AND** `specforge.diffLayout` holds `split` in that surface's `localStorage`

#### Scenario: The choice survives a restart

- **WHEN** the reader chooses side by side in the desktop app, quits, relaunches it and opens a commit
- **THEN** the commit's diff opens side by side

#### Scenario: Surfaces keep their own choice

- **WHEN** side by side is stored in the desktop app and the reader chooses unified in a phone's browser tab of a served instance
- **THEN** the desktop app's next diff still opens side by side
- **AND** no application setting changes

#### Scenario: An open view keeps its layout

- **WHEN** two tabs of one served instance show commit detail in unified, and the reader chooses side by side in the first
- **THEN** the second tab's diff stays unified
- **AND** the next commit the second tab opens starts side by side

#### Scenario: A re-read keeps the open view's layout

- **WHEN** a pull request's diff is open side by side in one window, unified is then chosen in another window of the same surface, and the pull request is re-read in the first window
- **THEN** the first window's diff stays side by side

#### Scenario: Only exactly split reads as side by side

- **WHEN** the stored value is `Split`, `split ` or `side-by-side`
- **THEN** a diff opens unified

#### Scenario: A failed write keeps the choice for the view only

- **WHEN** the reader chooses side by side and the store refuses the write
- **THEN** the view re-lays out side by side
- **AND** the next diff the surface opens starts in the layout stored before

#### Scenario: The layout is in no address

- **WHEN** the reader switches the layout
- **THEN** the Address and the window's URL are unchanged

#### Scenario: The layout is chosen only in the diff view

- **WHEN** the reader opens Settings or the application menu
- **THEN** neither offers a choice of diff layout

### Requirement: Narrow Views Fall Back to Unified

Side by side SHALL be in effect only while it is chosen and the file sections are wide enough for two columns. The width SHALL be that of the column the file sections occupy, never the window's, measured in `ch` of the code font, so the thresholds follow the code font's size and the zoom. Side by side SHALL come into effect at 104 ch and leave below 96 ch. Each time the width or the choice changes, with $$w$$ the width, chosen meaning side by side is the chosen layout, $$s$$ whether side by side was in effect until then and $$s'$$ whether it is from then on:

$$s' \iff \text{chosen} \wedge \big(w \ge 104 \;\vee\; (s \wedge w \ge 96)\big)$$

The gap between the two thresholds keeps a scrollbar that appears after a switch from flipping the layout straight back.

The width SHALL be known before the rows first paint, so a diff never paints in one layout and then flips to the other as it opens, and SHALL be kept current as the column or the code font's size changes.

While the fallback holds, the control SHALL keep "Side by side" checked and SHALL say "Too narrow — showing unified", as visible text that is also the control's accessible description. The stored choice SHALL NOT be rewritten, so widening the view brings side by side back.

While side by side is chosen, the navigator SHALL also fold above the sections wherever keeping it beside them would leave the sections column under 104 ch, so widening the view never turns side by side off. That SHALL be decided from the view's width less the navigator's, measured in the code font's `ch` like the thresholds, never in the view's own font.

#### Scenario: Side by side holds between the thresholds

- **WHEN** side by side is chosen and in effect at a sections width of 110 ch, and the column narrows to 100 ch
- **THEN** side by side stays in effect
- **WHEN** the column narrows to 95 ch
- **THEN** the view shows unified
- **WHEN** the column widens back to 100 ch
- **THEN** the view still shows unified
- **WHEN** the column widens to 104 ch
- **THEN** side by side is in effect again

#### Scenario: The fallback says why and keeps the choice

- **WHEN** side by side is chosen and the sections column is 80 ch wide
- **THEN** the view shows unified
- **AND** the control keeps "Side by side" checked and shows "Too narrow — showing unified", which is also its accessible description
- **AND** the stored choice is still `split`

#### Scenario: The window's width does not decide the layout

- **WHEN** the window is 1,600 CSS pixels wide but the side panes leave the sections column under 96 ch of the code font
- **THEN** the view shows unified

#### Scenario: Zoom moves the threshold

- **WHEN** side by side is in effect with the sections column 120 ch wide, and the reader zooms in until the same column measures 90 ch of the code font
- **THEN** the view shows unified

#### Scenario: No flash of the wrong layout

- **WHEN** a diff opens with side by side chosen in a view whose sections column is 80 ch wide
- **THEN** its first painted rows are unified rows
- **AND** no side-by-side row is painted before them

#### Scenario: Widening never turns side by side off

- **WHEN** side by side is chosen and in effect with the navigator folded above the sections, and the reader widens the view until the navigator would fit beside the sections but leave them under 104 ch
- **THEN** the navigator stays folded above the sections
- **AND** side by side stays in effect

#### Scenario: Unified chosen keeps the navigator beside the sections

- **WHEN** unified is chosen and the view is wide enough for the navigator beside the sections, though the sections beside it are under 104 ch
- **THEN** the navigator sits beside the sections

### Requirement: Selection and Copying

Side by side, a selection SHALL stay in the column it started in. A pointer-down in a side-by-side code cell SHALL name that cell's side for the whole view, and every file's grid SHALL then refuse selection in the other column, so a drag into the next file stays on the same side. The side SHALL stay named until a pointer-down outside a code cell, or until a pointer-up leaves the selection collapsed or outside the view; the collapsed selection a pointer-down leaves before a drag SHALL NOT clear it.

Copying a selection whose two ends lie in code cells of one file, while unified is in effect or a side is named, SHALL put text built from the model on the clipboard, never text read from the page: side by side, the named side's selected lines; in unified, the selected lines in the order shown. That text SHALL be code text only, with no line numbers, markers, fillers or badges, and SHALL honour a partial first and last line.

Any other selection (one with an end in a file header, in a preamble such as a review thread, or in a hunk row; one spanning files; or, side by side, one made while no side is named) SHALL copy its text in document order, still leaving out line numbers, markers, fillers and badges.

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

### Requirement: Syntax Highlighting

Diff lines SHALL be highlighted by the file's language, with the same highlighter and grammars as fenced code in rendered markdown. The language SHALL come from the file name's extension, or from its whole name for an extensionless file such as `Makefile`, resolved against the grammar set markdown code blocks use by default. A language that set does not know SHALL render plain.

Each hunk SHALL be highlighted as two contiguous texts, its old side (its context and removed lines) and its new side (its context and added lines), and each result split back into lines, so a comment or string that spans lines inside a hunk is highlighted correctly. Side by side, the left column SHALL show the old side's tokens and the right column the new side's. In unified, a removed line SHALL show the old side's tokens, an added line the new side's, and a context line the new side's: the code as it now reads. Highlighting SHALL be done in the frontend, so the model carries text only.

Once tokens own the text colour, added and removed lines SHALL be told apart by a background tint and their markers, in both layouts. Every token colour, whether a literal or drawn from a token, SHALL clear the palette's 4.5:1 contrast floor on the code well and on the added, removed and context line backgrounds, in both schemes and both layouts; where one value cannot clear every background, the diff view SHALL give that colour a value per scheme and per background (see the *Syntax Highlight Palette* requirement in the `visual-identity` capability). A filler cell holds no text, so the floor does not apply to it: it SHALL be told from an empty line by its missing line number and its own neutral background token.

#### Scenario: A line inside a block comment is highlighted as a comment

- **WHEN** a hunk's first context line opens a block comment and a removed line two lines below it lies inside the comment
- **THEN** the removed line is highlighted as a comment, in both layouts

#### Scenario: Unified context lines read as the code now reads

- **WHEN** a hunk adds `/*` directly above a context line `x = 1;` and `*/` directly below it
- **THEN** side by side, the context line's left cell is highlighted as code and its right cell as a comment
- **AND** in unified, the context line is highlighted as a comment

#### Scenario: An extensionless file is highlighted by its name

- **WHEN** a diff changes a file named `Makefile`
- **THEN** its lines are highlighted as a makefile

#### Scenario: An unknown language renders plain

- **WHEN** a diff changes `notes.qqq`, whose extension names no known language
- **THEN** its lines render as plain text, with their markers and tints

#### Scenario: Token colours keep the floor on tinted lines

- **WHEN** a highlighted removed line holds a comment, in either scheme
- **THEN** the comment's colour measures at least 4.5:1 against the removed-line background

#### Scenario: A filler is told from an empty line

- **WHEN** side by side shows an empty removed line facing a filler
- **THEN** the empty line shows its number and its marker on the removed background
- **AND** the filler shows no number, on its own background

### Requirement: Hidden Characters Are Shown

The diff view SHALL render every character with the Unicode property Default_Ignorable_Code_Point as a visible, marked escape, in diff lines, in each hunk header's section heading, in paths and in the side names a host passes. That set includes the bidirectional controls, the zero-width characters, the tag characters, variation selectors and Hangul fillers, the soft hyphen and the invisible operators.

On a context line, in a path, in a side name, and in a title or branch name a host shows with these escapes, only these SHALL be exempt and render as themselves:

- a U+200D between two emoji;
- one U+FE0E or U+FE0F directly after an emoji character;
- the U+FE0F of a keycap sequence: `0`–`9`, `#` or `*`, then U+FE0F, then U+20E3;
- the tag characters of the three RGI subdivision flags: U+1F3F4, then the tag characters spelling `gbeng`, `gbsct` or `gbwls`, then U+E007F;
- a U+FEFF at the very start of a context line that is both old line 1 and new line 1.

Here an emoji character is one with the Unicode property Extended_Pictographic. A U+200D is between two emoji when an emoji character directly follows it and an emoji character precedes it, either directly or followed by one emoji modifier (U+1F3FB–U+1F3FF) or one U+FE0F. A U+200D or a variation selector after a digit, `#` or `*` is therefore escaped, save the U+FE0F of a keycap sequence.

Every other variation selector or tag character SHALL be escaped, so a run of them after an emoji is never hidden. On an added or removed line nothing SHALL be exempt, so a change that only adds or removes such a character never shows as two identical lines.

A section heading SHALL take the exemptions of a context line, and a character escaped for itself there SHALL raise its file's warning.

A file SHALL carry a warning in its header when its lines contain a character escaped for itself. A character escaped only because its line is added or removed, that is one the exemptions would leave unescaped were the line context (a U+FEFF counting so at the very start of a line that is line 1 of its side), SHALL show its escape but SHALL raise no warning.

Escapes and the warning SHALL be decided from a line's own text, kind and line numbers, never from the layout or the facing cell, so a line shows the same escapes in both layouts and in either column, and a file's warning does not depend on the layout.

#### Scenario: A bidirectional control is escaped and warned of

- **WHEN** an added line contains U+202E
- **THEN** it renders as a visible, marked escape
- **AND** the file's header carries the warning

#### Scenario: A Hangul filler on a context line is escaped

- **WHEN** a context line holds U+3164 inside an identifier
- **THEN** it renders as an escape, and the file's header carries the warning

#### Scenario: Emoji sequences on a context line render as themselves

- **WHEN** a context line holds a family emoji joined by U+200D and a heart followed by one U+FE0F
- **THEN** both render as emoji, with nothing escaped and no warning

#### Scenario: A skin-toned or flag ZWJ sequence on a context line renders as itself

- **WHEN** a context line holds a technologist with a skin-tone modifier (U+1F468, U+1F3FD, U+200D, U+1F4BB) and the rainbow flag (U+1F3F3, U+FE0F, U+200D, U+1F308)
- **THEN** both render as emoji, with nothing escaped and no warning

#### Scenario: A joiner between two digits is escaped and warned of

- **WHEN** a context line holds `1`, U+200D and `2`
- **THEN** the U+200D renders as an escape, and the file's header carries the warning

#### Scenario: A keycap renders as itself

- **WHEN** a context line holds the keycap `1`, U+FE0F, U+20E3
- **THEN** it renders as a keycap, with nothing escaped and no warning
- **WHEN** an added line holds the same keycap
- **THEN** its U+FE0F is escaped and raises no warning
- **WHEN** a context line holds `#` followed by U+FE0F, with no U+20E3 after them
- **THEN** the U+FE0F is escaped, and the file's header carries the warning

#### Scenario: The same emoji on a changed line are escaped without a warning

- **WHEN** an added line holds the same family emoji and heart
- **THEN** each U+200D and the U+FE0F render as escapes
- **AND** they raise no warning

#### Scenario: A change of only a variation selector is visible

- **WHEN** a change replaces a line with the same line plus one U+FE0F after an emoji
- **THEN** the removed and the added lines read differently, the added line showing the escape

#### Scenario: A second variation selector is escaped on a context line

- **WHEN** a context line holds an emoji followed by U+FE0F twice
- **THEN** the second U+FE0F is escaped, and the file's header carries the warning

#### Scenario: Only the three subdivision flags pass as flags

- **WHEN** a context line holds U+1F3F4, the tag characters spelling `gbsct`, and U+E007F
- **THEN** the flag renders as itself, with no warning
- **WHEN** a context line holds U+1F3F4 followed by tag characters spelling any other text
- **THEN** every one of those tag characters is escaped, and the file's header carries the warning

#### Scenario: A leading byte-order mark is exempt only at line 1 of both sides

- **WHEN** a context line that is old line 1 and new line 1 starts with U+FEFF
- **THEN** the U+FEFF renders as itself
- **WHEN** an added line that is new line 1 starts with U+FEFF
- **THEN** it is escaped and raises no warning
- **WHEN** a context line that is old line 1 and new line 3 starts with U+FEFF
- **THEN** it is escaped, and the file's header carries the warning

#### Scenario: Paths and side names are escaped

- **WHEN** a file's path contains U+200B and a host passes a side name containing U+202E
- **THEN** the path shows the escape in the file's header and in the navigator
- **AND** the side name shows the escape in the toolbar while side by side is in effect

#### Scenario: A bidirectional control in a hunk heading is escaped and warned of

- **WHEN** a hunk's header carries a section heading that contains U+202E, in a file whose lines hold no escaped character
- **THEN** the heading shows U+202E as a visible, marked escape, in both layouts
- **AND** the file's header carries the warning

#### Scenario: Escapes do not depend on the layout

- **WHEN** a diff with escaped characters on context, removed and added lines is shown in unified and then side by side
- **THEN** each line shows the same escapes in both layouts and in either column
- **AND** each file's header warning is the same in both layouts

### Requirement: Side-Qualified Line Identity and Switching

Every rendered line SHALL carry a side-qualified identity, in both layouts: old line n for a removed line, new line n for an added line, and both for a context line. The identity SHALL NOT depend on the layout, so content anchored to a side and a line number, as GitHub's `diffSide` and BitBucket's `inline.from` and `inline.to` give one, finds its line in either layout.

Switching the layout SHALL keep the reader's place by the identity of the topmost visible line: the line at the top of the view before the switch SHALL be at the top after it. A switch SHALL keep, per file, every section's collapse, every withheld file already loaded with its hunks, and the navigator's mark. A switch SHALL re-read nothing, invoking no command and starting no `git` process, and SHALL re-tokenise nothing: each hunk's rows and token lines are computed once and reused by both layouts.

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

### Requirement: Diff View Hosts

The diff view SHALL be the one view through which every host shows a diff, such as commit detail or a pull request's view, so no host forks the renderer. A host SHALL pass it the model, the names of the two sides and a loader. Two optional per-file slots SHALL carry everything else a host adds:

- the **header extra**, rendered inside the file's sticky header, where a pull request's "viewed" mark would sit;
- the **preamble**, rendered between the header and the first hunk, across the section's full width, where a pull request's review threads would sit.

Neither slot SHALL be given a column, and both SHALL render the same in either layout.

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

### Requirement: Keyboard and Accessibility

The layout control SHALL be exposed to assistive technology as a radio group of two options. Its checked option SHALL be its single Tab stop, and the arrow keys SHALL move to and select the next or previous option, wrapping at either end and switching the layout as a click does. It SHALL be visible at rest without hover, as *The Layout Choice Is Per Surface* requires, and while the fallback holds it SHALL expose the fallback's reason as its accessible description (see the *Narrow Views Fall Back to Unified* requirement). The navigator SHALL be keyboard-operable as the *File Navigator* requirement says.

Each two-column side-by-side file grid SHALL carry visually hidden column headers naming its columns: old line, old, new line and new; a file in one column SHALL carry the two that name its side. Every line-number cell, in both layouts, SHALL be named by its side, its number and its line's kind, for example "old line 12, removed". Fillers SHALL be hidden from assistive technology. Reading order SHALL follow visual order in both layouts, because each layout is its own structure: unified reads line by line in the model's order, and side by side row by row, the left half before the right.

Unified SHALL remain fully equivalent for anyone who reads linearly: every line, with its numbers, marker, no-newline badge and escapes, and every slot, appears in it.

#### Scenario: The layout control is one Tab stop with wrapping arrows

- **WHEN** the reader tabs to the layout control while "Unified" is checked
- **THEN** focus lands on "Unified", and the next Tab leaves the control
- **WHEN** the reader presses ArrowRight
- **THEN** "Side by side" is focused and checked, and the view switches to side by side
- **WHEN** the reader presses ArrowRight again
- **THEN** focus wraps to "Unified", which is checked, and the view switches back to unified

#### Scenario: Line numbers are named by side and kind

- **WHEN** assistive technology reaches the number cell of removed old line 12
- **THEN** the cell is named "old line 12, removed"

#### Scenario: Side-by-side grids carry hidden column headers

- **WHEN** side by side is in effect and a file renders in two columns
- **THEN** its grid exposes the column headers old line, old, new line and new to assistive technology
- **AND** the headers are not visible

#### Scenario: Fillers are not announced

- **WHEN** assistive technology reads a side-by-side row whose right half is a filler
- **THEN** it reads the removed line and nothing for the filler

#### Scenario: The control is visible without hover

- **WHEN** the served web UI is loaded on a device that reports no hover capability
- **THEN** the layout control is visible at rest, with its text labels

#### Scenario: Unified reads linearly

- **WHEN** assistive technology reads a hunk in unified
- **THEN** it reads the hunk's lines in the model's order, each line's numbers named by side and kind
