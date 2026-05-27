## MODIFIED Requirements

### Requirement: Accent Color

The application SHALL use Linear indigo `#5e6ad2` as its single accent color, exposed as the `--accent` token, with a hover variant `--accent-hover` of `#4f5bbf` and a low-opacity tint `--accent-tint` of `rgba(94, 106, 210, 0.10)`. The accent SHALL be used for selection emphasis, focus rings, primary-action buttons, and inline markdown links. The raw macOS system blue (`rgb(0, 122, 255)`) MUST NOT appear in user-visible chrome.

#### Scenario: Selected tree row uses the accent

- **WHEN** a row in the workspace tree is selected
- **THEN** the row renders a 2px left border in `--accent`
- **AND** the row background is unchanged by the selected state (selection lives entirely in the inline-start border slot)

#### Scenario: Primary button uses the accent

- **WHEN** a primary button is rendered (for example "Add workspace" in settings)
- **THEN** its background is `--accent`
- **AND** its hover background is `--accent-hover`

#### Scenario: Links in rendered markdown use the accent

- **WHEN** the detail pane renders an `<a>` element from markdown
- **THEN** the link color is `--accent`

### Requirement: Tree Row Selection Model

A selected row in the workspace tree (and in any other list surface that conforms to the row grammar, such as the settings workspaces list) SHALL render a 2px solid `--accent` left border and no background change relative to its unselected state. Hover state SHALL render `background: var(--surface-2)` uniformly on every row, regardless of depth or whether the row is a top-level workspace row. Keyboard focus SHALL render an `outline: 2px solid var(--accent)` with `outline-offset: -2px`.

The previous selection treatment that composed an `--accent-tint` background fill MUST NOT be used; the 2px accent left bar is the sole selection signal. The previous hover treatment that composed `--surface-2` over a workspace tint via `background-blend-mode: multiply` is no longer applicable because top-level rows no longer carry a tint background (see the `spec-browser` capability's `Top-Level Row Display Name and Swatch` requirement); hover SHALL render `var(--surface-2)` on every row, no composition.

#### Scenario: Selected row in the tree

- **WHEN** the user clicks an unselected tree row
- **THEN** the row renders a 2px left bar in `--accent`
- **AND** the row background does not change relative to the unselected state — every row keeps the default row background regardless of depth

#### Scenario: Hover does not borrow selection styling

- **WHEN** the user hovers over an unselected tree row
- **THEN** the row background is `--surface-2` on every row, regardless of depth or whether the row is a top-level workspace row
- **AND** the row does not render an accent left bar
