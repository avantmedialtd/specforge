## MODIFIED Requirements

### Requirement: Design Token Layer

The application SHALL expose a single source of design tokens as CSS custom properties declared on `:root`, with dark-scheme overrides scoped to `@media (prefers-color-scheme: dark)`. All UI chrome (sidebar, tree rows, detail pane, settings, badges, buttons, dividers) SHALL consume these tokens rather than inline literal color, size, spacing, or radius values.

The token set SHALL include, at minimum:

- Color: `--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`, `--accent`, `--accent-hover`, `--accent-tint`, `--ok`, `--warn`.
- Type sizes: `--text-xs` (12px), `--text-sm` (13px), `--text-base` (14px), `--text-md` (15px), `--text-lg` (16px), `--text-xl` (22px), `--text-2xl` (30px).
- Type families: `--font-ui`, `--font-mono`.
- Line heights: `--leading-tight` (1.5), `--leading-prose` (1.65), `--leading-code` (1.5).
- Space: `--space-1` (4px), `--space-2` (8px), `--space-3` (12px), `--space-4` (16px), `--space-5` (24px), `--space-6` (32px), `--space-7` (48px).
- Radii: `--radius-sm` (4px), `--radius` (6px), `--radius-md` (8px).

The pre-existing legacy custom properties `--row-hover`, `--row-selected`, `--text-muted` (in its previous untyped form), `--divider`, and `--divider-hover` SHALL be removed once their callers consume the new tokens.

#### Scenario: Tokens defined on :root

- **WHEN** the application stylesheet is loaded
- **THEN** `:root` declares the full color, type, space, radii, and border token set
- **AND** every UI rule in the stylesheet references at least one token rather than a literal value (color literals are permitted only inside token definitions themselves)

#### Scenario: Type-size tokens render at the retuned px values

- **WHEN** the application stylesheet is loaded
- **THEN** `--text-xs` resolves to 12px, `--text-sm` to 13px, `--text-base` to 14px, `--text-md` to 15px, `--text-lg` to 16px, `--text-xl` to 22px, and `--text-2xl` to 30px
- **AND** `--leading-tight` resolves to 1.5

#### Scenario: Dark mode token overrides

- **WHEN** the operating system reports `prefers-color-scheme: dark`
- **THEN** the dark-scheme media query overrides at least `--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, and `--text-faint`
- **AND** the accent and status tokens remain consistent across light and dark

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` with the outlined-chip treatment (`border: 1px solid var(--border)`, transparent background, `--radius-sm`), and fenced code blocks in `--font-mono` with `--leading-code` (1.5). The maximum content width SHALL be 880px — chosen so that body prose at `--text-lg` (16px) renders roughly 100 characters per line, and fenced code blocks at `--text-md` (15px mono) render roughly 97 characters per line. This balances a single-column prose measure with sufficient horizontal room for code-block content typical of OpenSpec proposals and specs on a 4K display at 100% OS scale.

Markdown-rendering changes beyond typography (callouts, anchor links, custom code-block chrome) are explicitly out of scope and MUST NOT be introduced by this change.

#### Scenario: Body text uses Inter at the prose size

- **WHEN** the detail pane renders any markdown paragraph
- **THEN** the paragraph computed `font-family` is `--font-ui`
- **AND** the computed `font-size` is `--text-lg`
- **AND** the computed `line-height` is `--leading-prose`

#### Scenario: Inline code is an outlined chip

- **WHEN** the detail pane renders a `<code>` element that is not inside a `<pre>`
- **THEN** the element renders with a 1px `--border` outline
- **AND** the element background is transparent
- **AND** the element font-family is `--font-mono`

## ADDED Requirements

### Requirement: List-Row Vertical Rhythm Tuned for 4K @ 100%

The vertical padding of list-row-like surfaces SHALL be set so that rows read comfortably on a 4K display at 100% OS scale (one CSS px = one device px) without losing the dense-browser character of the sidebar. Specifically:

- The workspace tree row (`.tree-row`) SHALL use 5px vertical padding (top and bottom).
- The settings workspaces row (`.workspace-row`) SHALL use `--space-4` (16px) vertical padding.
- The settings toggle row (`.settings-toggle-row`) SHALL use 6px vertical padding.

The horizontal padding of these rows is unchanged by this requirement; only vertical rhythm is constrained.

#### Scenario: Tree row breathes at the retuned padding

- **WHEN** a workspace tree row is rendered
- **THEN** the row's computed `padding-top` and `padding-bottom` are both 5px
- **AND** the row's horizontal padding values are unchanged from the existing layout

#### Scenario: Settings workspaces row tracks the tree row

- **WHEN** a registered-workspace row is rendered in settings
- **THEN** the row's computed vertical padding resolves from `--space-4`
- **AND** the row does not feel visually tighter than a sidebar tree row at the same display scale
