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
- a withheld, too-large, binary or hunk-less file SHALL show the same full-width state row, with the same loader where one is offered, and a withheld file loaded while side by side is in effect SHALL render side by side. An image file SHALL instead use its own comparison modes, independent of the text layout, as the *Image Comparison* requirement says;
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
  3. `notImage`: its bytes begin with none of the PNG, JPEG, GIF, WebP, BMP, ICO and AVIF signatures, its header cannot be read, or it is a PNG, or an icon holding a PNG, whose first chunk is not `IHDR`;
  4. `tooManyPixels`: its width times its height exceeds 40,000,000, or so does that of any other canvas a decoder may size it by. These are $$C(s)$$: a GIF's screen grown to hold each frame at its offset; each image an icon holds, by that image's own header; and each frame header a JPEG decoder finds, reading past fill bytes, standalone markers and stray bytes as libjpeg does.

$$\text{tooManyPixels}(s) \iff \max_{c \,\in\, \{s\} \cup C(s)} w(c) \cdot h(c) > 40\,000\,000$$

**What may decode.** The view SHALL decode only an image side. A refused side SHALL never reach an image element. Bytes SHALL render from memory, through an object URL that the view creates for each mounted image and revokes when the image unmounts. No image SHALL be loaded from a URL that names a host.

**When a file reads.** Each host says when its image files read. A file read **as its section nears the view** SHALL be read once its section comes within one viewport height of the visible area, with at most two such reads in flight per view, taken in the order their sections came near. A section that leaves that margin before its read starts SHALL be dropped from the queue, so a section never brought near the view reads nothing. A file read **on request** SHALL show a "Show image" control, and SHALL read when the reader activates it.

**Results.**
- **Keeping a read.** The view SHALL keep what a read returned, per file, until the host passes a new model. Collapsing and expanding the section, switching the layout and switching the mode SHALL read nothing again.
- **A read that cannot complete.** It SHALL show the reader's reason beside a control that tries again: "Show image" when the host reads on request, "Try again" when it reads near the view.
- **No image at all.** A file whose sides are all absent or refused as `notImage` SHALL show the state row it would show if it were not an image file.

**Rendering.**
- **Layouts.** Image comparisons SHALL be independent of text diff layout. Side by side SHALL be the default, old left and new right. An added or deleted file SHALL show its one version across the section's full width. A refused or undrawable side SHALL retain its note beside any drawable version.
- **Captions.** Each version SHALL be captioned with Before or After, its side's name with the escapes the *Hidden Characters Are Shown* requirement gives side names, then its width and height and its size. When both sides are images, the new side's caption SHALL add the change in size, as bytes and as a whole percentage of the old size. Composite modes SHALL retain both versions' metadata.
- **Sizes.** A size SHALL be written in bytes below 1 KB, and otherwise in KB or MB of 1,024 units: one decimal below 10, none from 10 up ("812 B", "345 KB", "1.4 MB").
- **Drawing.** Versions SHALL share one scale and top-left alignment within their union extents. Each inline version SHALL be drawn over a checkerboard, at its natural size and never larger, scaled down to fit. Its box SHALL be reserved before it decodes. Different dimensions SHALL NOT stretch a version to match the other.
- **What a side shows instead of its image:**

| Side | It reads |
|---|---|
| refused `lfs` | "Stored in Git LFS" |
| refused `tooLarge` | "Too large to preview" |
| refused `notImage` | "Not an image this view can show" |
| refused `tooManyPixels` | "Too many pixels to preview" |
| an image the view cannot draw | "Can't be shown here" |

**Modes.** Comparison modes SHALL follow the *Image Comparison Modes*, *Bounded Highlighted Image Diffs* and *Interactive Image Comparison* requirements. The mode radio group SHALL follow the *Keyboard and Accessibility* requirement.

**The reader's place.** When an image file's read completes and its comparison replaces its placeholder, the view SHALL keep the reader's place as the *Folded Hunks* requirement keeps it for a change from elsewhere: the topmost visible line stays where it was.

**Accessibility and copying.**
- **Names.** Each version SHALL be named by its file's path and its side's name ("icons/app.png at abc1234"). Composite frames SHALL be named by their mode, path and both side names.
- **Copying.** Captions SHALL copy as text in document order. Image-loading, zoom, mode, slider and blink controls SHALL be left out of every copy, as the controls of a folded hunk are (see the *Selection and Copying* requirement).

**Zooming.** A host MAY let its image files zoom. Each comparison of such a host SHALL then offer a "Zoom" control, and SHALL treat a click on a version as the same request, to open the file's versions in a **window of their own**: a native window in the desktop application, and a tab in the browser skin. Operating a slider or blink control SHALL NOT open a window.
- **One window per file.** Asking again for a file whose window is open SHALL focus that window rather than open another.
- **What it reads.** The window SHALL be addressed by what names the versions, never by their bytes: a commit's repository, commit and paths, or a pull request's reference, path and commits, with the two side names. It SHALL read the versions through the same command its host's image reader uses, governed as that read is, and SHALL check them as this requirement says.
- **What it shows.** The window SHALL offer the same six modes on the same terms, defaulting to Side by side with an independent choice; or the available versions and refusal notes.
- **Lockstep.** Both frames SHALL share one scale and one position. Zooming or panning either moves both, so the same pixel of each version sits at the same place in its frame, with both versions aligned at their top-left corners. A mode switch SHALL retain the scale and the first frame's position.
- **Gestures.** The window SHALL follow the platform's gestures:
  - two-finger scrolling on a trackpad, and the mouse wheel, SHALL pan, with the platform's momentum;
  - a pinch SHALL zoom, anchored at the pointer, as SHALL the wheel with Control;
  - dragging SHALL pan, and on a touch screen two contacts SHALL pinch; dragging the slider divider SHALL adjust the wipe instead;
  - Command (Control elsewhere) with `+` or `=` and with `-` SHALL zoom in and out, with `0` SHALL show actual size, and with `9` SHALL fit;
  - Escape and Command-W (Control-W elsewhere) SHALL close the window.
- **Controls.** Zoom in, zoom out, fit, actual size and close, beside the current scale. It SHALL open at fit, every version wholly visible.
- **Bounds.** With $$s_{\text{fit}}$$ taken over the extents that hold both versions, the scale SHALL stay within $$\bigl[\min(s_{\text{fit}}, 1),\ 32\bigr]$$.
- **Pixels.** Above actual size, each version and generated diff SHALL be drawn with its pixels kept square and sharp, never smoothed.
- **Safety.** The window SHALL load nothing from a URL that names a host, under the content-security policy of a pull-request window, and SHALL be granted no permission beyond closing itself. Opening it SHALL change nothing in the diff it was opened from.

#### Scenario: Two versions sit side by side
- **WHEN** a commit modifies `icons/app.png`, a 512 × 512 PNG of 345 KB that became 298 KB
- **THEN** its section defaults to the old version on the left and the new on the right, each over a checkerboard
- **AND** the old caption reads Before, its side's name, "512 × 512" and "345 KB", and the new caption adds "−47 KB (−14%)"

#### Scenario: Unified stacks the versions
- **WHEN** unified is in effect and an image file is modified
- **THEN** its default image comparison is still Side by side
- **AND** changing the text layout neither changes the image mode nor reads the versions again

#### Scenario: An added image shows one version
- **WHEN** a commit adds `icons/new.png`
- **THEN** its section shows only the new version across its full width in either layout
- **AND** no two-image mode choice is offered

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

#### Scenario: A canvas the header hides is refused
- **WHEN** an image file's new version is a GIF whose screen declares 3 × 2 and whose first frame declares 30,000 × 30,000
- **THEN** that side reads "Too many pixels to preview", and its bytes are never decoded

#### Scenario: Bytes decide, not the name
- **WHEN** an image file `icons/app.png` holds JPEG bytes
- **THEN** it renders as a JPEG image
- **AND** a side whose bytes match no listed signature reads "Not an image this view can show"

#### Scenario: A binary file that is no image keeps its state row
- **WHEN** a read finds both sides of `data/blob.png` refused as `notImage`
- **THEN** its section shows "Binary file not shown", as any binary file's does

#### Scenario: Different dimensions preserve alignment
- **WHEN** an image file's old version is 512 × 512 and its new version is 1024 × 1024
- **THEN** all six comparison modes are offered
- **AND** the old version occupies its original half-width and half-height at the same scale, without stretching

#### Scenario: JPEG orientation preserves the decoded aspect ratio
- **WHEN** an accepted JPEG has raw dimensions 200 × 100 and EXIF orientation 6
- **THEN** inline frames, zoom frames, captions and pixel comparison use its oriented dimensions of 100 × 200 without stretching
- **AND** the orientation-aware union is checked against the pixel-work limit before allocating comparison canvases

#### Scenario: Difference needs equal dimensions
- **WHEN** an image file's old version is 512 × 512 and its new version is 1024 × 1024
- **THEN** the legacy black-background Difference mode is not offered
- **AND** the replacement Diff, Slider and Blink modes are offered without an equal-dimensions restriction

#### Scenario: Difference shows unchanged pixels as black
- **WHEN** the reader chooses Diff for an icon whose versions differ only in a 48 × 48 square
- **THEN** changed pixels in the square are highlighted over a muted unchanged image
- **AND** a text summary reports the changed pixel count and percentage

#### Scenario: Difference is not remembered
- **WHEN** the reader chooses Diff for one image file and the host then passes a new model
- **THEN** that file shows Side by side
- **AND** nothing about the choice was written to the application settings or to the surface's storage

#### Scenario: A section far from the view reads nothing
- **WHEN** a host reads image files as their sections near the view, and a commit lists twenty image files of which only the first two are within one viewport height of the visible area
- **THEN** only those two files are read
- **AND** no read is made for a file whose section the reader never brings near the view

#### Scenario: At most two reads are in flight
- **WHEN** five image files' sections come near the view at once
- **THEN** two reads are in flight, and each of the other three starts as one ends

#### Scenario: Collapsing and switching read nothing again
- **WHEN** an image file has been read, and the reader collapses and expands its section, switches the layout, and chooses Diff
- **THEN** no further read is made for it

#### Scenario: A read on request waits for the reader
- **WHEN** a host reads image files on request
- **THEN** each image file shows "Show image" and no read is made until the reader activates it

#### Scenario: An image loading above the reader keeps their place
- **WHEN** new line 80 of a file is the topmost visible line, and an image file's read completes in a section above it
- **THEN** new line 80 is still the topmost visible line

#### Scenario: A version the view cannot draw says so
- **WHEN** a side is an image whose type the platform's web view cannot draw
- **THEN** that side reads "Can't be shown here", the other side still shows its image, and no two-image modes remain active

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

#### Scenario: Before and After isolate their version
- **WHEN** the reader chooses Before and then After
- **THEN** one full-width frame shows only the selected version with its metadata, without another provider read

#### Scenario: Zoom mode switches preserve the inspected region
- **WHEN** the reader pans a zoomed comparison and switches from Side by side to Slider and back
- **THEN** the scale and the first frame's scroll position are retained, with the second frame following when it returns

#### Scenario: A wider mode clamps without losing the retained position
- **WHEN** the reader pans near a zoomed comparison's right edge, switches to Slider and then back to Side by side
- **THEN** temporary browser scroll clamping in the wider Slider viewport does not overwrite the retained position, and both returning frames restore that position
- **AND** if the reader pans while in Slider, the new position is retained instead

## ADDED Requirements

### Requirement: Image Comparison Modes

Two drawable images, even of unequal sizes, SHALL offer Diff, Before, After, Side by side, Slider and Blink independently of text layout. The per-file mode SHALL default to Side by side, stay unpersisted and reset with a new model. Its visible radio group SHALL have one Tab stop and arrow-key selection. Before/After SHALL isolate one version full-width at the common scale. Missing, refused or undrawable sides SHALL fall back to available versions and notes without two-image modes.

#### Scenario: Unequal versions expose every comparison mode
- **WHEN** both versions are drawable images with different dimensions
- **THEN** the visible mode group offers Diff, Before, After, Side by side, Slider and Blink
- **AND** Side by side is selected initially regardless of the text diff layout

#### Scenario: The mode group has one keyboard stop
- **WHEN** the reader tabs to the image mode group and presses an arrow key
- **THEN** the selected choice is the group's only Tab stop, and the arrow key moves focus and selection as the text layout control does

#### Scenario: Individual modes isolate the selected version
- **WHEN** the reader chooses Before or After
- **THEN** one full-width frame shows only that version at the comparison's common scale

#### Scenario: Mode state belongs to one file and model
- **WHEN** the reader chooses a mode for one image file and the host later passes a new model
- **THEN** other files' choices are unaffected, and the new model starts that file at Side by side
- **AND** the choice is never written to application settings or surface storage

#### Scenario: An unavailable side ends an active two-image mode
- **WHEN** an image version becomes undrawable while a two-image mode is selected, or a version is missing or refused
- **THEN** the view falls back to the available versions and their refusal or undrawable notes
- **AND** it offers no two-image mode group

### Requirement: Bounded Highlighted Image Diffs

Diff SHALL compute locally on demand from accepted image bytes, highlight changed pixels over muted unchanged content and report changed count/percentage. It SHALL compare top-left-aligned union extents, distinguish added/removed area from transparency, and bound pixel processing separately from image acceptance. Failure, cancellation or an oversized comparison SHALL preserve every other mode and SHALL NOT trigger another provider read.

#### Scenario: Identical pixels
- **WHEN** two images decode to identical pixels and dimensions
- **THEN** Diff reports zero changed pixels

#### Scenario: Transparency and resized edges
- **WHEN** versions have transparent pixels and different dimensions
- **THEN** Diff compares visible pixel content consistently and highlights added or removed image area even if transparent

#### Scenario: Pixel-work limit
- **WHEN** the union comparison canvas exceeds 4,000,000 pixels
- **THEN** Diff shows a processing-limit explanation without allocating that canvas
- **AND** Before, After, Side by side, Slider and Blink remain available

#### Scenario: Processing fails or the model changes
- **WHEN** local decoding or processing fails, or an in-flight diff's model is replaced
- **THEN** failures are explained locally, stale results cannot replace the current comparison, and transient resources are released

### Requirement: Interactive Image Comparison

Slider SHALL reveal After to the left of a movable divider and Before to its right, without stretching. The divider SHALL support pointer/touch and keyboard operation with an accessible percentage. Blink SHALL alternate versions only when selected and visible, offer Pause/Resume, start paused under reduced motion, and release its timer when inactive. Neither interaction SHALL change review state or read versions again.

#### Scenario: Slider endpoints and keyboard
- **WHEN** the reader drags the divider or uses arrow keys, Home or End
- **THEN** its accessible reveal percentage follows the wipe, clamped to 0–100
- **AND** 0 shows Before and 100 shows After
- **AND** operating the divider neither pans the zoom frame nor opens another window

#### Scenario: Blink is explicit and pausable
- **WHEN** Blink is selected without reduced motion
- **THEN** it alternates Before and After at 500 ms intervals and offers Pause
- **AND** Pause freezes the current version and Resume restarts alternation

#### Scenario: Reduced motion and hidden content
- **WHEN** reduced motion is requested, or the comparison becomes hidden or leaves the viewport
- **THEN** reduced motion starts Blink paused until explicitly resumed, and hidden content does not alternate
- **AND** leaving Blink or unmounting releases its timer
