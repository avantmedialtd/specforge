## MODIFIED Requirements

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` as unboxed colour-tinted text (a dedicated `--code-fg` colour at `font-weight: 500`, with no fill, border, or radius), and fenced code blocks in `--font-mono` with `--leading-code` (1.5). The maximum content width SHALL be 880px — chosen so that body prose at `--text-lg` renders roughly 100 characters per line and fenced code at `--text-md` renders roughly 97 characters per line.

Inline `<code>` (a `<code>` element not inside a `<pre>`) SHALL render as unboxed text — no background fill, no border, and no border-radius — distinguished from body prose by `--font-mono`, a `font-weight` of 500, and a dedicated `--code-fg` colour token. `--code-fg` SHALL be a hue distinct from `--accent` (which colours markdown links), so inline code and links remain separable even where they sit together, with the mono family reinforcing the distinction. `--code-fg` SHALL be defined per scheme and SHALL clear the AA 4.5:1 contrast floor against the markdown background in both schemes — a darker shade on light, a brighter shade on dark. The same unboxed-text recipe SHALL be used for `.settings-help code`, so the application has ONE inline-code recipe.

Fenced code blocks (`pre`) SHALL render as a lifted well: `--surface` background, 1px `--border`, `--radius`, and `--shadow-2` (which includes the inner top-light), distinct from the unboxed inline code. The `pre code` element SHALL remain transparent and borderless. Blockquotes SHALL render as `--surface-3` aside cards with a 3px accent-at-0.7 left rule and `--text-muted` body.

Markdown-rendering changes beyond typography and the elevation/aside treatments described here (callouts, anchor links, custom code-block chrome) are explicitly out of scope and MUST NOT be introduced by this change.

#### Scenario: Body text uses Inter at the prose size

- **WHEN** the detail pane renders any markdown paragraph
- **THEN** the paragraph computed `font-family` is `--font-ui`
- **AND** the computed `font-size` is `--text-lg`
- **AND** the computed `line-height` is `--leading-prose`

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
