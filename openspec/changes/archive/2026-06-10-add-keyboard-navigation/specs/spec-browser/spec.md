## ADDED Requirements

### Requirement: Workspace Tree Keyboard Navigation

The workspace tree SHALL be fully operable from the keyboard as a WAI-ARIA tree with a roving tabindex: the tree occupies exactly one position in the window's Tab order, and within it a single current row carries focus, movable with the keyboard. Keyboard activation SHALL reuse the same selection contract as pointer clicks — a row whose click renders content in the detail pane renders the same content when activated by keyboard, and rows whose clicks are disclosure-only remain disclosure-only.

#### Scenario: Tree is a single Tab stop with a roving current row

- **WHEN** the user presses Tab from the control preceding the tree (or Shift+Tab from the control following it)
- **THEN** focus lands on the tree's current row — the last row focused in this session, or the first visible row if none — rather than entering every row in sequence
- **AND** pressing Tab again moves focus out of the tree to the next control in the window's Tab order

#### Scenario: Arrow keys traverse visible rows

- **WHEN** the tree has focus and the user presses ArrowDown or ArrowUp
- **THEN** focus moves to the next or previous visible row in rendered order, crossing workspace boundaries, without wrapping at either end
- **AND** the newly focused row scrolls into view if it is outside the sidebar's viewport

#### Scenario: Home and End jump to the extremes

- **WHEN** the tree has focus and the user presses Home or End
- **THEN** focus moves to the first or last visible row of the tree

#### Scenario: ArrowRight and ArrowLeft drive disclosure and parent jumps

- **WHEN** the user presses ArrowRight on a collapsed expandable row
- **THEN** the row expands, honouring the same expansion-persistence behavior as a chevron click
- **WHEN** the user presses ArrowRight on an already-expanded row
- **THEN** focus moves to the row's first child
- **WHEN** the user presses ArrowLeft on an expanded row
- **THEN** the row collapses
- **WHEN** the user presses ArrowLeft on a collapsed or leaf row that has a parent row
- **THEN** focus moves to the parent row

#### Scenario: Enter and Space activate the current row

- **WHEN** the user presses Enter or Space on a row whose pointer click renders content in the detail pane (instance, proposal/design/tasks artifact, capability-spec, section, and task rows)
- **THEN** the row is selected and the detail pane renders exactly what a pointer click on that row would render
- **WHEN** the user presses Enter or Space on a disclosure-only grouping row (workspace, repo, logical change, and change rows, plus the Specs artifact row — whose pointer click also renders no content)
- **THEN** the row's expansion toggles, identically to a chevron click

#### Scenario: Debounced follow-focus opens content without per-keystroke reads

- **WHEN** keyboard focus comes to rest on a row whose pointer click renders content in the detail pane, and remains there for a short settle delay (approximately 150 ms)
- **THEN** the detail pane renders that row's content as if the row had been activated
- **WHEN** focus passes over such rows more quickly than the settle delay (for example while an arrow key is held down)
- **THEN** no intermediate row's content is loaded or rendered
- **WHEN** keyboard focus rests on a disclosure-only grouping row
- **THEN** the detail pane does not change

#### Scenario: First-letter typeahead

- **WHEN** the tree has focus and the user types a printable character
- **THEN** focus moves to the next visible row after the current one whose label starts with that character, comparing case-insensitively and wrapping past the end of the tree
- **AND** if no visible row label starts with that character, focus does not move

#### Scenario: Tree rows expose ARIA tree semantics

- **WHEN** the tree is rendered
- **THEN** the container exposes `role="tree"`, every row exposes `role="treeitem"` with an accurate `aria-level`, expandable rows expose `aria-expanded` reflecting their disclosure state, the selected row exposes `aria-selected="true"`, and nested child groups are wrapped in `role="group"` containers
- **AND** dim missing-artifact rows remain keyboard-focusable but expose `aria-disabled="true"` and do not respond to activation

#### Scenario: Focus survives the focused row disappearing

- **WHEN** a tree refresh (for example a filesystem cache event) removes the row that currently holds keyboard focus
- **THEN** focus falls back to the nearest surviving ancestor row derived from the removed row's hierarchical node ID, rather than being lost to the document body

#### Scenario: Keyboard focus movement does not re-render the whole tree

- **WHEN** the user moves keyboard focus between rows
- **THEN** only the rows whose visual state changed re-render; unaffected subtrees are not re-rendered

### Requirement: Shell Keyboard Operability

The browsing shell around the tree SHALL be keyboard-operable: split-pane dividers MUST be focusable and resizable from the keyboard, the Settings and Archive panes MUST be dismissible with Escape, and every keyboard-focusable control in the shell MUST show a visible focus indicator when focused via keyboard, using the visual-identity spec's keyboard-focus recipe.

#### Scenario: Dividers resize from the keyboard

- **WHEN** a split-pane divider receives keyboard focus and the user presses ArrowLeft or ArrowRight
- **THEN** the adjacent pane resizes by a fixed step per keypress, respecting the same minimum-width limits as a pointer drag, and the divider exposes `role="separator"` with `aria-valuenow`, `aria-valuemin`, and `aria-valuemax` reflecting the current and permitted sizes

#### Scenario: Escape dismisses Settings and Archive

- **WHEN** the Settings pane or the Archive pane is open and the user presses Escape (with no text input focused that consumes it)
- **THEN** the open pane closes and the detail pane returns to what it previously displayed

#### Scenario: Focusable controls show visible keyboard focus

- **WHEN** any focusable control in the sidebar, archive view, graph rail, or settings view receives focus via keyboard
- **THEN** it renders a visible focus indicator per the visual-identity keyboard-focus recipe
- **AND** focus styles use `:focus-visible` so pointer clicks do not paint focus rings
