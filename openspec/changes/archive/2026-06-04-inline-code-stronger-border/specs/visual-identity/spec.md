# visual-identity

## MODIFIED Requirements

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` with the outlined-chip treatment (`border: 1px solid var(--border-strong)`, transparent background, `--radius-sm`), and fenced code blocks in `--font-mono` with `--leading-code` (1.5). The maximum content width SHALL be 880px — chosen so that body prose at `--text-lg` renders roughly 100 characters per line and fenced code at `--text-md` renders roughly 97 characters per line.

Inline `<code>` SHALL remain an outlined chip with a 1px `--border-strong` and a TRANSPARENT background — it SHALL NOT take a `--surface-2` fill. The `--border-strong` token (not the lighter decorative `--border` hairline) is used so the chip outline carries enough weight to read as code against body prose. The same transparent-chip recipe SHALL be used for `.settings-help code`, so the application has ONE inline-code recipe.

Fenced code blocks (`pre`) SHALL render as a lifted well: `--surface` background, 1px `--border`, `--radius`, and `--shadow-2` (which includes the inner top-light), distinct from the flat transparent inline chip. The `pre code` element SHALL remain transparent and borderless. Blockquotes SHALL render as `--surface-3` aside cards with a 3px accent-at-0.7 left rule and `--text-muted` body.

Markdown-rendering changes beyond typography and the elevation/aside treatments described here (callouts, anchor links, custom code-block chrome) are explicitly out of scope and MUST NOT be introduced by this change.

#### Scenario: Body text uses Inter at the prose size

- **WHEN** the detail pane renders any markdown paragraph
- **THEN** the paragraph computed `font-family` is `--font-ui`
- **AND** the computed `font-size` is `--text-lg`
- **AND** the computed `line-height` is `--leading-prose`

#### Scenario: Inline code is an outlined chip

- **WHEN** the detail pane renders a `<code>` element that is not inside a `<pre>`
- **THEN** the element renders with a 1px `--border-strong` outline
- **AND** the element background is transparent
- **AND** the element font-family is `--font-mono`

#### Scenario: Fenced code block is a lifted well

- **WHEN** the detail pane renders a `<pre>` fenced code block
- **THEN** it renders with a `--surface` background, a 1px `--border`, and `--shadow-2`
- **AND** the inner `pre code` element is transparent and borderless
