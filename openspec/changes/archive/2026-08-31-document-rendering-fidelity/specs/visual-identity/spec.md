## ADDED Requirements

### Requirement: Markdown Block Rhythm

Every block-level element of the markdown view SHALL take its vertical margins from the application stylesheet: no rendered block may ride a user-agent default margin or a vendor stylesheet margin. Block spacing SHALL form exactly two authored tiers — a prose tier for paragraphs and lists, and a wider object tier shared by every non-prose block (fenced code, display mathematics, tables, diagrams, SVG images, blockquotes) — so a document that interleaves prose with objects presents one consistent rhythm rather than a different gap per element kind. In particular, fenced code blocks (whose margin today is the user-agent `<pre>` default) and display mathematics (whose margin today comes from the bundled KaTeX stylesheet) SHALL be given the object tier's margin by the application stylesheet.

The GFM footnotes section SHALL be visually separated from the document body: a top rule, object-tier or larger top spacing, and footnote text rendered smaller and more muted than body prose. Footnote definitions SHALL NOT read as a continuation of the last paragraph.

#### Scenario: Code and math join the object tier

- **WHEN** the markdown view renders a fenced code block and a display-mathematics block
- **THEN** each computes the same authored vertical margin as a table or a diagram block
- **AND** neither margin comes from a user-agent or vendor stylesheet

#### Scenario: Footnotes are set off from the body

- **WHEN** an artifact uses GFM footnotes
- **THEN** the footnotes section renders below a separating rule with clear top spacing
- **AND** its text is smaller and more muted than body prose

### Requirement: Markdown Table Presentation

Table header cells SHALL render on a surface tier visually distinct from every body-row fill, including the alternating zebra fill, so the header row is identifiable by more than font weight alone. Table cell text SHALL render at `--text-md` — one step below body prose — so dense tables gain column room without a change in table structure. Zebra striping of body rows SHALL be preserved.

#### Scenario: The header row is distinct from zebra rows

- **WHEN** the markdown view renders a GFM table with three or more rows
- **THEN** the header cells' fill differs from the fill of every body row, striped and unstriped alike
- **AND** header text remains bolder than body-cell text

#### Scenario: Table text is one step denser than prose

- **WHEN** the markdown view renders a GFM table
- **THEN** its cell text computes to `--text-md`
- **AND** surrounding body prose remains at `--text-lg`

### Requirement: Syntax Highlight Palette

The fenced-code syntax-highlight palette SHALL be scheme-aware: every token class the palette colours (keywords, strings, numbers, types, comments, titles) SHALL clear the AA 4.5:1 contrast floor against the code well's background in BOTH the light and the dark scheme. Palette colours MAY be literal values (the *Design Token Layer* requirement's syntax-palette carve-out stands), but a literal SHALL then be defined per scheme rather than shared, whenever one value cannot clear the floor on both wells.

#### Scenario: Token colours clear AA on the light code well

- **WHEN** the light scheme is active and the markdown view renders a highlighted fence containing strings, numbers, keywords, and types
- **THEN** each token colour measures at least 4.5:1 against the code well background

#### Scenario: Token colours clear AA on the dark code well

- **WHEN** the dark scheme is active and the markdown view renders the same fence
- **THEN** each token colour measures at least 4.5:1 against the dark code well background

## MODIFIED Requirements

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` as unboxed colour-tinted text (a dedicated `--code-fg` colour at `font-weight: 500`, with no fill, border, or radius), and fenced code blocks in `--font-mono` with `--leading-code` (1.5).

The content column SHALL be two-tier. The column itself SHALL remain 880px at maximum — the width available to object blocks (tables, fenced code, diagrams, SVG images, display mathematics), chosen so that fenced code at `--text-md` renders roughly 97 characters per line. Prose blocks — paragraphs, lists, and blockquotes — SHALL additionally be limited to a readable measure between 70ch and 80ch of their own font, so body text wraps near the typographic comfort range rather than at the roughly 110 characters the full column would produce. Headings SHALL keep the full column, so the hairline rules beneath `h1` and `h2` span the whole reading surface; object blocks SHALL likewise keep the full column so wide content does not pay for the prose measure.

The object tier SHALL win wherever the two nest. A prose block that CONTAINS an object block — a fenced code block indented under a numbered step, a table inside a blockquote — SHALL NOT impose the measure on it, since a nested element cannot exceed the width of the block containing it. The narrowing SHALL be lifted from the containing block only, at the granularity of the individual list item, so that plain-prose items in the same list keep the measure.

Inline `<code>` (a `<code>` element not inside a `<pre>`) SHALL render as unboxed text — no background fill, no border, and no border-radius — distinguished from body prose by `--font-mono`, a `font-weight` of 500, and a dedicated `--code-fg` colour token. `--code-fg` SHALL be a hue distinct from `--accent` (which colours markdown links), so inline code and links remain separable even where they sit together, with the mono family reinforcing the distinction. `--code-fg` SHALL be defined per scheme and SHALL clear the AA 4.5:1 contrast floor against the markdown background in both schemes — a darker shade on light, a brighter shade on dark. The same unboxed-text recipe SHALL be used for `.settings-help code`, so the application has ONE inline-code recipe.

Fenced code blocks (`pre`) SHALL render as a lifted well: `--surface` background, 1px `--border`, `--radius`, and `--shadow-2` (which includes the inner top-light), distinct from the unboxed inline code. The `pre code` element SHALL remain transparent and borderless. Blockquotes SHALL render as `--surface-3` aside cards with a 3px accent-at-0.7 left rule and `--text-muted` body.

Markdown-rendering treatments beyond typography, the elevation/aside treatments described here, the block rhythm, table, and syntax-palette treatments (see the *Markdown Block Rhythm*, *Markdown Table Presentation*, and *Syntax Highlight Palette* requirements), and the task-checkbox treatment (see the *Markdown Task-Checkbox Treatment* requirement) — callouts, anchor links, custom code-block chrome — remain out of scope of this requirement and are not sanctioned by it.

#### Scenario: Body text uses Inter at the prose size

- **WHEN** the detail pane renders any markdown paragraph
- **THEN** the paragraph computed `font-family` is `--font-ui`
- **AND** the computed `font-size` is `--text-lg`
- **AND** the computed `line-height` is `--leading-prose`

#### Scenario: Prose wraps at the readable measure while objects keep the column

- **WHEN** the detail pane renders a long paragraph followed by a wide table in a pane wider than the content column
- **THEN** the paragraph's text wraps at a measure between 70ch and 80ch
- **AND** the table may extend to the full 880px column

#### Scenario: An object nested in a list keeps the full column

- **WHEN** a list item contains a fenced code block, and a sibling item in the same list contains only prose
- **THEN** the fenced code block renders at the full content column, not at the prose measure
- **AND** the prose-only sibling item still wraps at the measure

#### Scenario: Inline code is unboxed coloured text

- **WHEN** the detail pane renders a `<code>` element that is not inside a `<pre>`
- **THEN** the element has no background fill, no border, and no border-radius
- **AND** the element font-family is `--font-mono`
- **AND** the element font-weight is 500
- **AND** the element colour is `--code-fg`, a hue distinct from `--accent`

#### Scenario: Fenced code block is a lifted well

- **WHEN** the detail pane renders a `<pre>` fenced code block
- **THEN** it renders with a `--surface` background, a 1px `--border`, and `--shadow-2`
- **AND** the inner `pre code` element is transparent and borderless
