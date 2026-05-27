## MODIFIED Requirements

### Requirement: Accent Color

The application SHALL use Linear indigo `#5e6ad2` as its single accent color, exposed as the `--accent` token, with a hover variant `--accent-hover` of `#4f5bbf` and a low-opacity tint `--accent-tint` of `rgba(94, 106, 210, 0.10)`. The accent SHALL be used for selection emphasis, focus rings, primary-action buttons, and inline markdown links. The raw macOS system blue (`rgb(0, 122, 255)`) MUST NOT appear in user-visible chrome.

#### Scenario: Selected tree row uses the accent

- **WHEN** a row in the workspace tree is selected
- **THEN** the row renders a 2px left border in `--accent`
- **AND** the row background is unchanged by the selected state (the workspace tint, if any, remains visible underneath the selection bar; the default row background otherwise)

#### Scenario: Primary button uses the accent

- **WHEN** a primary button is rendered (for example "Add workspace" in settings)
- **THEN** its background is `--accent`
- **AND** its hover background is `--accent-hover`

#### Scenario: Links in rendered markdown use the accent

- **WHEN** the detail pane renders an `<a>` element from markdown
- **THEN** the link color is `--accent`

### Requirement: Tree Row Selection Model

A selected row in the workspace tree (and in any other list surface that conforms to the row grammar, such as the settings workspaces list) SHALL render a 2px solid `--accent` left border and no background change relative to its unselected state. Hover state SHALL render `background: var(--surface-2)` only on untinted rows; tinted top-level rows SHALL compose the hover wash over the tint as defined by the workspace-tint requirements in the spec-browser capability. Keyboard focus SHALL render an `outline: 2px solid var(--accent)` with `outline-offset: -2px`.

The previous selection treatment that composed an `--accent-tint` background fill (and, on tinted top-level rows, a linear-gradient of `--accent-tint` over the workspace tint) MUST NOT be used; the 2px accent left bar is the sole selection signal.

#### Scenario: Selected row in the tree

- **WHEN** the user clicks an unselected tree row
- **THEN** the row renders a 2px left bar in `--accent`
- **AND** the row background does not change relative to the unselected state — a tinted top-level row keeps its workspace tint visible underneath the selection bar, and an untinted row keeps the default row background

#### Scenario: Hover does not borrow selection styling

- **WHEN** the user hovers over an unselected tree row
- **THEN** the row background is `--surface-2` (on untinted rows) or the existing hover composition over the workspace tint (on tinted rows)
- **AND** the row does not render an accent left bar

## ADDED Requirements

### Requirement: Flat Tree Row Geometry

The workspace tree row (`.tree-row`) SHALL render without a `border-radius` and without an inline-axis margin (no side gutter between the row and the sidebar edge). The row's tint background, hover background, and selection left bar SHALL therefore fill the row edge-to-edge across the full sidebar width.

This geometry SHALL apply uniformly to every tree row regardless of depth or tint state, so tinted top-level rows and untinted child rows share the same horizontal footprint and the row grammar remains uniform across the tree.

The existing 2px inline-start transparent border (used to reserve space for the selection bar so selected and unselected rows do not shift horizontally) SHALL be preserved — only the corner radius and outer inline margin are removed.

#### Scenario: Tree row renders edge-to-edge

- **WHEN** a workspace tree row is rendered
- **THEN** the row's computed `border-radius` is `0`
- **AND** the row's computed inline-axis margin (left and right) is `0`
- **AND** the row's tint or hover background extends from the sidebar's inline-start edge to its inline-end edge without any gutter

#### Scenario: Tinted and untinted rows share row geometry

- **WHEN** a tinted top-level workspace row is rendered above an untinted child row
- **THEN** both rows resolve to the same `border-radius` (`0`) and the same inline margin (`0`)
- **AND** the visible difference between them is only the tint background on the parent, not the row footprint
