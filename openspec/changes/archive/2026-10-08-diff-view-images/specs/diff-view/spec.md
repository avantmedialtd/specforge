## ADDED Requirements

### Requirement: Image Comparison

**Image files.** An **image file** is a changed file that meets both of these:
- **Its path.** Its new path, or its old path when it was deleted, ends in `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.ico`, `.bmp` or `.avif`, ignoring case.
- **Its content.** It is binary, holds no hunks, or holds nothing but the lines of a Git LFS pointer.

Every other file renders as the other requirements of this capability say. That includes an SVG, and a file named like an image whose content is withheld, too large, or textual hunks. With $$E$$ the eight extensions:

$$\text{image}(f) \iff \text{ext}(f) \in E \;\wedge\; \bigl(\text{binary}(f) \;\vee\; \text{hunks}(f) = \varnothing \;\vee\; \text{lfs}(f)\bigr)$$

**Git LFS pointers.** A Git LFS pointer is text of under 1,024 bytes. Its first line is `version https://git-lfs.github.com/spec/v1`, and its other lines are an `oid sha256:` line with 64 hexadecimal digits, a `size` line with a decimal number, and any `ext-` lines Git LFS allows. An image file whose every hunk line belongs to such a pointer, on whichever side it stands, SHALL read "Stored in Git LFS", SHALL offer no control, and SHALL cause no read.

**Reading.** Every other image file SHALL show its two versions. They SHALL be read through its host's image reader (see the *Diff View Hosts* requirement), never taken from its content. The reader SHALL answer each side as exactly one of:

- an **image**: its type, sniffed from its bytes; its width and height, read from its header alone; and its bytes;
- **absent**: the old side of an added file, or the new side of a deleted one;
- **refused**, with the first reason that applies, in this order:
  1. `lfs`: the side is a Git LFS pointer;
  2. `tooLarge`: the side is past 8 MiB;
  3. `notImage`: its bytes begin with none of the PNG, JPEG, GIF, WebP, BMP, ICO and AVIF signatures, or its header cannot be read;
  4. `tooManyPixels`: its width times its height exceeds 40,000,000.

$$\text{tooManyPixels}(s) \iff w(s) \cdot h(s) > 40\,000\,000$$

**What may decode.** The view SHALL decode only an image side. A refused side SHALL never reach an image element. Bytes SHALL render from memory, through an object URL that the view creates for each mounted image and revokes when the image unmounts. No image SHALL be loaded from a URL that names a host.

**When a file reads.** Each host says when its image files read. A file read **as its section nears the view** SHALL be read once its section comes within one viewport height of the visible area, with at most two such reads in flight per view, taken in the order their sections came near. A section that leaves that margin before its read starts SHALL be dropped from the queue, so a section never brought near the view reads nothing. A file read **on request** SHALL show a "Show image" control, and SHALL read when the reader activates it.

**Results.**
- **Keeping a read.** The view SHALL keep what a read returned, per file, until the host passes a new model. Collapsing and expanding the section, switching the layout and switching the mode SHALL read nothing again.
- **A read that cannot complete.** It SHALL show the reader's reason beside a control that tries again: "Show image" when the host reads on request, "Try again" when it reads near the view.
- **No image at all.** A file whose sides are all absent or refused as `notImage` SHALL show the state row it would show if it were not an image file.

**Rendering.**
- **Layouts.** Side by side, the old version SHALL sit in the left half and the new in the right. In unified they SHALL stack, old above new. An added or deleted file SHALL show its one version across the section's full width, in both layouts.
- **Captions.** Each version SHALL be captioned with its side's name, as the toolbar names it and with the escapes the *Hidden Characters Are Shown* requirement gives side names, then its width and height and its size. When both sides are images, the new side's caption SHALL add the change in size, as bytes and as a whole percentage of the old size.
- **Sizes.** A size SHALL be written in bytes below 1 KB, and otherwise in KB or MB of 1,024 units: one decimal below 10, none from 10 up ("812 B", "345 KB", "1.4 MB").
- **Drawing.** Each version SHALL be drawn over a checkerboard, at its natural size and never larger, scaled down to fit its column. Its box SHALL be reserved from its width and height before it decodes.
- **What a side shows instead of its image:**

| Side | It reads |
|---|---|
| refused `lfs` | "Stored in Git LFS" |
| refused `tooLarge` | "Too large to preview" |
| refused `notImage` | "Not an image this view can show" |
| refused `tooManyPixels` | "Too many pixels to preview" |
| an image the view cannot draw | "Can't be shown here" |

**Difference.** When both sides are images of equal width and equal height, the section SHALL offer a choice of "Both", the default, and "Difference".
- **What Difference shows.** One frame across the section's full width, on black, with the new version blended over the old by difference, so that every unchanged pixel is black. Its caption says so.
- **The control.** It SHALL be a radio group with one Tab stop, operated by the arrow keys as the layout control is (see the *Keyboard and Accessibility* requirement).
- **The choice.** It SHALL be view state held per file. It SHALL NOT be a preference, SHALL NOT be persisted, SHALL be dropped when the host passes a new model, and SHALL render the same in either layout.

**The reader's place.** When an image file's read completes and its comparison replaces its placeholder, the view SHALL keep the reader's place as the *Folded Hunks* requirement keeps it for a change from elsewhere: the topmost visible line stays where it was.

**Accessibility and copying.**
- **Names.** Each version SHALL be named by its file's path and its side's name ("icons/app.png at abc1234"). The Difference frame SHALL be named by its path and both sides' names.
- **Copying.** Captions SHALL copy as text in document order. The "Show image", "Try again", zoom and mode controls SHALL be left out of every copy, as the controls of a folded hunk are (see the *Selection and Copying* requirement).

**Zooming.** A host MAY let its image files zoom. Each comparison of such a host SHALL then offer a "Zoom" control, and SHALL treat a click on a version as the same request, to open the file's versions in a **window of their own**: a native window in the desktop application, and a tab in the browser skin.
- **One window per file.** Asking again for a file whose window is open SHALL focus that window rather than open another.
- **What it reads.** The window SHALL be addressed by what names the versions, never by their bytes: a commit's repository, commit and paths, or a pull request's reference, path and commits, with the two side names. It SHALL read the versions through the same command its host's image reader uses, governed as that read is, and SHALL check them as this requirement says.
- **What it shows.** Both versions in two equal frames side by side, old left, each captioned with its side's name; the one version of an added or deleted file; or, while Difference is chosen, the Difference frame. Difference SHALL be offered on the terms above, starting at Both, and the window's choice SHALL be its own.
- **Lockstep.** Both frames SHALL share one scale and one position. Zooming or panning either moves both, so the same pixel of each version sits at the same place in its frame, with both versions aligned at their top-left corners.
- **Gestures.** The window SHALL follow the platform's gestures:
  - two-finger scrolling on a trackpad, and the mouse wheel, SHALL pan, with the platform's momentum;
  - a pinch SHALL zoom, anchored at the pointer, as SHALL the wheel with Control;
  - dragging SHALL pan, and on a touch screen two contacts SHALL pinch;
  - Command (Control elsewhere) with `+` or `=` and with `-` SHALL zoom in and out, with `0` SHALL show actual size, and with `9` SHALL fit;
  - Escape and Command-W (Control-W elsewhere) SHALL close the window.
- **Controls.** Zoom in, zoom out, fit, actual size and close, beside the current scale. It SHALL open at fit, every version wholly visible.
- **Bounds.** With $$s_{\text{fit}}$$ taken over the extents that hold both versions, the scale SHALL stay within $$\bigl[\min(s_{\text{fit}}, 1),\ 32\bigr]$$.
- **Pixels.** Above actual size, each version SHALL be drawn with its pixels kept square and sharp, never smoothed.
- **Safety.** The window SHALL load nothing from a URL that names a host, under the content-security policy of a pull-request window, and SHALL be granted no permission beyond closing itself. Opening it SHALL change nothing in the diff it was opened from.

#### Scenario: Two versions sit side by side

- **WHEN** side by side is in effect and a commit modifies `icons/app.png`, a 512 × 512 PNG of 345 KB that became 298 KB
- **THEN** its section shows the old version in the left half and the new in the right, each over a checkerboard
- **AND** the old caption reads its side's name, "512 × 512" and "345 KB", and the new caption adds "−47 KB (−14%)"

#### Scenario: Unified stacks the versions

- **WHEN** unified is in effect and an image file is modified
- **THEN** its old version renders above its new version, each across the section's width

#### Scenario: An added image shows one version

- **WHEN** a commit adds `icons/new.png`
- **THEN** its section shows only the new version, across its full width, in either layout
- **AND** no Difference choice is offered

#### Scenario: A hunk-less image file shows its versions

- **WHEN** a pull request's files list `icons/app.png` with no hunks and no counted lines
- **THEN** it is an image file, and its section offers its versions in place of "No textual changes"

#### Scenario: A text file named like an image keeps its hunks

- **WHEN** a commit changes three lines of a text file named `notes.png`
- **THEN** its hunks render as any text file's do

#### Scenario: An SVG keeps its text diff

- **WHEN** a commit changes a path in `icons/logo.svg`
- **THEN** its hunks render as text, and no version is read or drawn

#### Scenario: A Git LFS pointer is labelled without a read

- **WHEN** an image file's only hunk replaces one Git LFS pointer's `oid` and `size` lines with another's
- **THEN** its section reads "Stored in Git LFS"
- **AND** it offers no control, and no read is made

#### Scenario: An oversized side is refused before it decodes

- **WHEN** the new version of an image file is 9 MiB
- **THEN** that side reads "Too large to preview" and no image element is made for it
- **AND** the old side still shows its image

#### Scenario: A pixel bomb is refused

- **WHEN** an image file's new version is a 40 KB PNG whose header declares 30,000 × 30,000 pixels
- **THEN** that side reads "Too many pixels to preview", and its bytes are never decoded

#### Scenario: Bytes decide, not the name

- **WHEN** an image file `icons/app.png` holds JPEG bytes
- **THEN** it renders as a JPEG image
- **AND** a side whose bytes match no listed signature reads "Not an image this view can show"

#### Scenario: A binary file that is no image keeps its state row

- **WHEN** a read finds both sides of `data/blob.png` refused as `notImage`
- **THEN** its section shows "Binary file not shown", as any binary file's does

#### Scenario: Difference needs equal dimensions

- **WHEN** an image file's old version is 512 × 512 and its new version is 1024 × 1024
- **THEN** no Difference choice is offered

#### Scenario: Difference shows unchanged pixels as black

- **WHEN** the reader chooses Difference for an icon whose versions differ only in a 48 × 48 square
- **THEN** one frame shows the icon black except where the square changed

#### Scenario: Difference is not remembered

- **WHEN** the reader chooses Difference for one image file and the host then passes a new model
- **THEN** that file shows Both
- **AND** nothing about the choice was written to the application settings or to the surface's storage

#### Scenario: A section far from the view reads nothing

- **WHEN** a host reads image files as their sections near the view, and a commit lists twenty image files of which only the first two are within one viewport height of the visible area
- **THEN** only those two files are read
- **AND** no read is made for a file whose section the reader never brings near the view

#### Scenario: At most two reads are in flight

- **WHEN** five image files' sections come near the view at once
- **THEN** two reads are in flight, and each of the other three starts as one ends

#### Scenario: Collapsing and switching read nothing again

- **WHEN** an image file has been read, and the reader collapses and expands its section, switches the layout, and chooses Difference
- **THEN** no further read is made for it

#### Scenario: A read on request waits for the reader

- **WHEN** a host reads image files on request
- **THEN** each image file shows "Show image" and no read is made until the reader activates it

#### Scenario: An image loading above the reader keeps their place

- **WHEN** new line 80 of a file is the topmost visible line, and an image file's read completes in a section above it
- **THEN** new line 80 is still the topmost visible line

#### Scenario: A version the view cannot draw says so

- **WHEN** a side is an image whose type the platform's web view cannot draw
- **THEN** that side reads "Can't be shown here", and the other side still shows its image

#### Scenario: Versions are named for assistive technology

- **WHEN** assistive technology reaches the old version of `icons/app.png` in a commit whose first parent is `abc1234`
- **THEN** it is named "icons/app.png at abc1234"

#### Scenario: Zoom opens the versions in their own window

- **WHEN** the reader activates "Zoom" on a modified icon in commit detail, or clicks one of its versions
- **THEN** a window of its own shows both versions side by side, fitted, old left
- **AND** activating "Zoom" on the same icon again focuses that window rather than opening another
- **AND** the diff it was opened from is unchanged and stays where it was

#### Scenario: Both versions zoom and pan together

- **WHEN** the reader pinches to zoom in with the pointer over a button in a screenshot's new version
- **THEN** both versions grow to the same scale, and the button stays beneath the pointer
- **AND** the old version shows the same region of its own pixels
- **AND** two-finger scrolling over either version moves both, with momentum

#### Scenario: Scrolling pans rather than zooms

- **WHEN** the reader scrolls with two fingers on a trackpad, or turns the mouse wheel, over a zoomed version
- **THEN** both versions move, and the scale is unchanged

#### Scenario: Keyboard zoom follows the platform

- **WHEN** the reader presses Command-plus twice, then Command-0, then Command-9 in the window on macOS
- **THEN** the versions zoom in twice, then show at actual size, then fit the window

#### Scenario: An enlarged icon shows its pixels

- **WHEN** the reader zooms a 16 × 16 icon's window to 1,600%
- **THEN** each pixel of both versions is drawn as a sharp square 16 pixels wide

#### Scenario: Zoom stops at 32 times

- **WHEN** the reader keeps zooming in
- **THEN** the scale stops at 3,200%
- **AND** zooming out stops once both versions are wholly visible, or at actual size when that is smaller

#### Scenario: A pull request's zoom window is a governed read

- **WHEN** the reader opens the zoom window of a pull request's image file whose merge base is known
- **THEN** the window reads the versions through `get_pull_request_file_image`, sending its two version requests and nothing else
- **AND** it renders them under the pull-request window's content-security policy

## MODIFIED Requirements

### Requirement: Line and Byte Budgets With On-Request Loading

Which files arrive with their hunks SHALL be decided by line and byte **budgets**, so the lines on the page stay bounded however large the diff, in either layout. The budgets SHALL decide only among **patched** files, those with a patch to show, taken in the model's file order. A binary file, and a file too large because it lies past a read ceiling that leaves it unreadable, SHALL keep its own state, SHALL add nothing to the line total, and SHALL never be withheld. A file whose provider omitted its patch for size, while reporting changed lines, SHALL instead arrive **withheld** whatever the budgets decide: its host reads it on request (see the *GitHub Detail Reads* requirement in the `pull-request-viewer` capability). It SHALL add nothing to the line total either.

With $$c(f)$$ the added plus removed lines of file $$f$$, context lines not counted, and $$g \prec f$$ when $$g$$ precedes $$f$$ in the model's file order, a file SHALL arrive with its hunks exactly when:

$$\text{eager}(f) \iff \text{patched}(f) \;\wedge\; c(f) \le 500 \;\wedge\; c(f) + \sum_{g \prec f,\ \text{eager}(g)} c(g) \le 3000$$

Every other patched file SHALL be withheld. Lines are not bytes, so two byte limits SHALL also apply as the patch text is read:

- a file SHALL be withheld once its own patch text passes 64 KiB;
- once the eager files' patch text reaches 1 MiB in total, every remaining file SHALL be withheld and reading SHALL stop. The file whose text reaches it keeps its hunks, so the eager text stays under 1 MiB plus one file's 64 KiB, and no patch text is read only to be discarded.

A file a byte limit withholds SHALL keep its lines in the line total, so the byte limits only ever shrink the eager set the line rule decided. A commit's streamed read SHALL give up once it has read 8 MiB of patch text in all, withholding every remaining patched file, each of which can still be read on its own (see the *Commit Detail View* requirement in the `commit-graph` capability). A host whose read ceiling leaves the files past it unreadable SHALL instead make those files too large to preview before the budgets apply. None of these limits SHALL be a setting, and none SHALL depend on the layout: the budgets are decided before any layout, and both layouts withhold the same files.

A withheld file SHALL reach the frontend with its counts and without its patch. It SHALL render collapsed, with its counts, and with a "Load diff" control in place of its lines. Activating the control SHALL read that file alone, through the host's loader, under a per-file ceiling of 8 MiB of diff text: the file SHALL then show its hunks or, past the ceiling, become too large to preview. A load that cannot complete SHALL leave the file withheld, show the loader's reason beside the control, and leave the control to try again. A too-large file SHALL read "too large to preview", and neither it nor a binary file SHALL offer a control to load its diff. An image file instead offers its two versions, read apart from its diff and counted by no budget (see the *Image Comparison* requirement). A host MAY give a too-large file, or an image file whose versions cannot all be shown, a way to view it elsewhere, in its per-file slots.

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

#### Scenario: Long lines wrap side by side and scroll in unified

- **WHEN** a hunk pairs a 400-character removed line with a short added line
- **THEN** side by side, the removed line wraps inside its cell with no number on its continuation, the row is as tall as the wrapped cell, and the added line starts level with it
- **AND** in unified, the line does not wrap, and the section's lines scroll sideways while its header stays in place

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

### Requirement: Diff View Hosts

The diff view SHALL be the one view through which every host shows a diff, such as commit detail or a pull request's view, so no host forks the renderer. A host SHALL pass it the model, the names of the two sides and a loader. Optional slots SHALL carry everything else a host adds, two per file and two per hunk:

- the **header extra**, rendered inside the file's sticky header, where a pull request's "viewed" mark would sit;
- the **preamble**, rendered between the header and the first hunk, across the section's full width, where a pull request's review threads would sit;
- the **hunk heading extra**, rendered in a hunk's heading row, in the gutter left of its `@@` heading, where a pull request's hunk mark would sit;
- the **hunk end**, rendered as a row after a hunk's last line, across the section's full width, only when the host gives it content for that hunk.

No slot SHALL be given a column, and each SHALL render the same in either layout. Beside its slots, a host MAY name the hunks it folds (see the *Folded Hunks* requirement), and MAY ask the view to collapse a file's section once (see the *File Sections* requirement). A host that passes no hunk slots and folds nothing SHALL see its hunks exactly as before: commit detail passes none.

**Image reader.** A host MAY also pass an **image reader**, which reads one image file's two versions (see the *Image Comparison* requirement). It SHALL say when those reads happen: **as a section nears the view**, or **on request**. Commit detail SHALL read as a section nears the view, and a pull request's view SHALL read on request. A host that passes no image reader SHALL see image files as any other files.

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
- **THEN** no hunk carries a heading extra or an end row, and no hunk is folded

#### Scenario: Each host reads its images when it says

- **WHEN** commit detail and a pull request's view each show a modified image file
- **THEN** commit detail reads it once its section nears the view, with no control
- **AND** the pull request's view shows "Show image" and reads it only when the reader activates that control

#### Scenario: Image files read only through the image reader

- **WHEN** a host shows an image file
- **THEN** its versions are read through that host's image reader, and its loader is never offered for it

