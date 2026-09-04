# visual-identity

## MODIFIED Requirements

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` as unboxed colour-tinted text (a dedicated `--code-fg` colour at `font-weight: 500`, with no fill, border, or radius), and fenced code blocks in `--font-mono` with `--leading-code` (1.5).

The content column SHALL be two-tier. The column itself — the width available to object blocks (tables, fenced code, diagrams, SVG images, display mathematics) — SHALL be bounded by the configured reading width, whose default is 880px, chosen so that fenced code at `--text-md` renders roughly 97 characters per line. Prose blocks — paragraphs, lists, and blockquotes — SHALL additionally be limited to a readable measure of their own font, defaulting to a measure between 70ch and 80ch, so body text wraps near the typographic comfort range rather than at the roughly 110 characters the default column would otherwise produce. Headings SHALL keep the full column, so the hairline rules beneath `h1` and `h2` span the whole reading surface; object blocks SHALL likewise keep the full column so wide content does not pay for the prose measure.

Both tiers SHALL be expressed as design tokens rather than as literal widths repeated per rule, so that one relationship is recorded in one place and the identity header above the document cannot fall out of alignment with the column it heads. Every declaration consuming those tokens SHALL carry an explicit fallback to the default rung, because a `max-width` whose custom property does not resolve is invalid at computed-value time and computes to `none` — silently removing the tier the declaration existed to impose.

Which reading width is in force is the `document-width` capability's concern. This requirement fixes the two-tier structure and its default rung; of every other reading width it requires only that the prose tier remain bounded and never exceed the object tier.

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

- **WHEN** the detail pane renders a long paragraph followed by a wide table in a pane wider than the content column, at the default reading width
- **THEN** the paragraph's text wraps at a measure between 70ch and 80ch
- **AND** the table may extend to the full 880px column

#### Scenario: The two tiers keep their relationship at every reading width

- **WHEN** any reading width offered by the `document-width` capability is in force
- **THEN** prose blocks are bounded to a measure rather than filling the column
- **AND** the prose tier is never wider than the object tier

#### Scenario: A tier whose token does not resolve falls back rather than disappearing

- **WHEN** a declaration's width token is unresolvable
- **THEN** the declaration takes the default rung's width through its own fallback
- **AND** neither the prose measure nor the object column is lost

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
