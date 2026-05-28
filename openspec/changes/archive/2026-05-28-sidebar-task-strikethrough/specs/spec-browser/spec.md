## ADDED Requirements

### Requirement: Leaf-Task Completion Rendering

The tree pane SHALL render each leaf-task row using only its label text, with no leading completion glyph in either the completed or the pending state. A completed task (a `- [x]` line) SHALL render its label with a line-through text decoration AND the faint/dimmed task text colour. A pending task (a `- [ ]` line) SHALL render its label with no text decoration in the default task-label colour. The completion state of a leaf task SHALL be conveyed by this text treatment alone, and SHALL NOT be conveyed by a leading checkbox or checkmark glyph.

This requirement governs leaf-task rows only. It SHALL NOT alter the aggregate completion indicators defined elsewhere in this capability: the trailing `✓` completion glyph on a fully-complete Section, flat-Change, and per-Instance row, and the `(completed/total)` task-progress label, all remain unchanged.

#### Scenario: Completed leaf task renders struck-through and dimmed

- **WHEN** a Section node is expanded and one of its task lines is `- [x]`
- **THEN** that task's row renders its label with a line-through text decoration and the dimmed task text colour
- **AND** no leading checkbox or checkmark glyph is rendered on the row

#### Scenario: Pending leaf task renders plain

- **WHEN** a Section node is expanded and one of its task lines is `- [ ]`
- **THEN** that task's row renders its label with no text decoration in the default task-label colour
- **AND** no leading checkbox or checkmark glyph is rendered on the row

#### Scenario: Aggregate completion indicators are retained

- **WHEN** every task in a Section is complete (and likewise for a fully-complete flat-Change row or per-Instance row)
- **THEN** the Section / flat-Change / Instance row continues to render its trailing `✓` completion glyph as before
- **AND** the `(completed/total)` task-progress label continues to render its count unchanged

#### Scenario: Selection composes with the strikethrough treatment

- **WHEN** a completed leaf-task row is the currently selected node
- **THEN** the row shows the standard selection treatment
- **AND** the row's label remains struck-through and rendered in the dimmed task text colour
