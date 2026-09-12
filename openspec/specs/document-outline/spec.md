# document-outline Specification

## Purpose

Defines the heading-derived outline that every shared markdown reading surface offers beside its prose column: hidden when the surface has no room to seat it, carrying per-section task progress on a live change's tasks document, and paired with stable heading identifiers so fragment links resolve within the rendered document.

## Requirements

### Requirement: Document Outline Surface

Every shared markdown reading surface — the detail pane rendering a change artifact, the archive reader, the workspace file browser's preview, and a reader window — SHALL offer an **outline** of the document it is rendering: one entry per heading of level two or three, in document order, with level-three entries visually subordinate to the level-two entry preceding them. Level-one headings are excluded, being the document's own title, and headings of level four and deeper are excluded, because a capability spec carries one per scenario and an outline of scenarios is as long as the document.

The outline SHALL be derived from the **rendered document's heading structure** — the same parse the renderer uses — never from a parallel parse of the source, so what the outline lists is exactly what the document shows, including headings that arrive by live refresh. A document with no heading of level two or three SHALL render no outline and reserve no space for one.

The outline is a **document** feature: it is rendered by the shared document view, so each surface receives it by rendering a document rather than by opting in, and a surface that later adopts the shared view inherits it.

#### Scenario: Headings become outline entries

- **WHEN** a document with several level-two headings, some followed by level-three headings, is rendered on a wide enough surface
- **THEN** the outline lists each level-two and level-three heading once, in document order
- **AND** each level-three entry is rendered subordinate to the level-two entry above it
- **AND** the document's level-one heading is not listed

#### Scenario: Deep headings are not listed

- **WHEN** a capability spec with level-four scenario headings is rendered
- **THEN** the outline lists the level-two and level-three headings only
- **AND** no scenario heading appears in it

#### Scenario: A document without headings has no outline

- **WHEN** a document with no heading of level two or three is rendered
- **THEN** no outline is rendered
- **AND** the prose column occupies the position it would occupy on a document that had one

#### Scenario: The outline follows a live refresh

- **WHEN** a rendered document is rewritten on disk with a new heading added
- **AND** the surface refreshes the document
- **THEN** the outline lists the new heading in its document position

### Requirement: Outline Placement and Visibility

The outline SHALL be seated **beside the prose column**, on its trailing side, inside the surface's scroll port, and SHALL stay in view while the document scrolls. It SHALL NOT overlap prose, objects, or the surface's header, and its presence SHALL NOT move the prose column: the column occupies the same horizontal position whether or not an outline is shown.

The outline SHALL be shown only when the surface has room to seat it beside the column at its full width; otherwise it SHALL be hidden entirely. There is no folded, collapsed or overlaid form. In particular, at the `full` rung of the reading-width ladder (see the `document-width` capability) the column has no trailing space and the outline is never shown. The determination SHALL respond to the surface's own width — a pane narrowed by the sidebar or the commit rail, a reader window resized by the user — rather than to the window's.

#### Scenario: The outline sits beside the column on a wide pane

- **WHEN** the detail pane is wide enough to seat the outline beside the prose column at the current reading width
- **THEN** the outline is rendered on the column's trailing side
- **AND** the column's horizontal position is the same as it is on a document with no outline

#### Scenario: A narrow pane hides the outline

- **WHEN** the detail pane is too narrow to seat the outline beside the prose column
- **THEN** no outline is rendered
- **AND** the document renders exactly as it would without the capability

#### Scenario: Widening the pane brings the outline back

- **WHEN** the outline is hidden because the pane is too narrow
- **AND** the user hides the sidebar or widens the window so the pane has room
- **THEN** the outline appears without the document being re-read

#### Scenario: The full reading width never shows an outline

- **WHEN** the reading width is set to `full`
- **THEN** no outline is rendered at any pane width

#### Scenario: The outline stays in view while reading

- **WHEN** the outline is shown and the user scrolls the document
- **THEN** the outline remains visible below the surface's sticky header

### Requirement: Outline Navigation

Activating an outline entry SHALL scroll the document so the corresponding heading comes to rest fully visible directly below the surface's sticky header, using the same measured-header clearance the surface's scroll anchors use (see *Change Identity Header in the Detail Pane* in the `spec-browser` capability, *Anchoring*). Activation SHALL NOT change the address, SHALL NOT create a history entry, and SHALL NOT re-read the document: it is a scroll, as *History Entry Discipline* in the `view-routing` capability treats scrolling.

The outline SHALL indicate the **current entry** — the entry whose heading most recently passed the top of the reading area — and SHALL update it as the reader scrolls, so the outline reads as a position indicator and not only as a menu.

Outline entries SHALL be reachable by keyboard in the surface's Tab order and activated by Enter and by Space, with the application's standard focus indicator. No global keyboard chord SHALL be introduced for the outline.

#### Scenario: Activating an entry scrolls to its heading

- **WHEN** the user activates an outline entry
- **THEN** the document scrolls so that heading rests fully visible directly below the sticky header
- **AND** the address and the history are unchanged

#### Scenario: The current entry follows the reader

- **WHEN** the user scrolls the document past a level-two heading
- **THEN** the outline marks that heading's entry as current
- **AND** the previously current entry is no longer marked

#### Scenario: Entries are keyboard-operable

- **WHEN** the user tabs to an outline entry and presses Enter or Space
- **THEN** the document scrolls exactly as a click on that entry would scroll it

### Requirement: Section Progress in a Tasks Outline

When the detail pane renders the **tasks artifact of a live change**, each outline entry whose heading text matches the title of a section parsed from that `tasks.md` (the `sections` the aggregation already carries in `ChangeData`) SHALL show that section's completed and total task counts. A section whose tasks are all complete SHALL show the completion glyph the change row uses (see *Change-Row Completion Glyph* in the `spec-browser` capability) in place of the counts. A heading with no matching parsed section, and every entry of any other document — a proposal, a design, a capability spec, an archived artifact, a file preview — SHALL show no counts.

The counts come from the parsed section model, not from the rendered document, so they agree with the progress meter on the change row and on the Tasks tab, which read the same model.

#### Scenario: A tasks outline shows per-section progress

- **WHEN** the detail pane renders the tasks artifact of a live change whose `tasks.md` parses three sections
- **THEN** each of the three outline entries shows its section's completed and total counts

#### Scenario: A complete section shows the completion glyph

- **WHEN** every task in a parsed section is complete
- **THEN** that section's outline entry shows the completion glyph and no counts

#### Scenario: Other documents show no counts

- **WHEN** the surface renders a proposal, a design, a capability spec, an archived change's tasks, or a file preview
- **THEN** no outline entry shows task counts

### Requirement: Fragment Links Resolve Within the Document

Every rendered heading SHALL carry a **stable identifier** derived from its text: lower-cased, with punctuation other than hyphens removed, whitespace replaced by single hyphens, and a numeric suffix (`-1`, `-2`, …) appended to later duplicates in document order — the same derivation common markdown hosts apply, so a fragment written for a document on such a host resolves here to the same heading.

A **fragment-only link** (`#…`) in a rendered document SHALL scroll to the heading carrying the matching identifier, under the same rule as an outline activation: no address change, no history entry, no re-read. This supersedes the sentence in *Link Handling in Rendered Artifacts* (`spec-browser`) that declared fragment-only links inert. A fragment that matches no heading SHALL be treated as a dangling link — a quiet indication that it could not be followed, with the document fully usable — and SHALL NOT navigate the application.

A link that carries a fragment together with a path is unaffected: it continues to be dispatched by its path's link class.

#### Scenario: A fragment link scrolls to its heading

- **WHEN** a rendered document contains a link to `#what-changes` and a level-two heading `What Changes`
- **THEN** clicking the link scrolls the document to that heading
- **AND** the address and the history are unchanged

#### Scenario: Duplicate headings get distinct identifiers

- **WHEN** a document contains two headings with the text `Scenario`
- **THEN** the first carries the identifier `scenario` and the second `scenario-1`
- **AND** a link to `#scenario-1` scrolls to the second

#### Scenario: A fragment matching no heading fails quietly

- **WHEN** the user clicks a fragment-only link whose fragment matches no heading in the document
- **THEN** a quiet indication is shown that the link could not be followed
- **AND** the document and the application view are unchanged
