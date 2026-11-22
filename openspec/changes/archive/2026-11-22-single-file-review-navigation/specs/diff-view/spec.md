## ADDED Requirements

### Requirement: Host-Selected File Visibility

A host SHALL be able to select one visible file without replacing the diff model. Other file sections SHALL be hidden from layout, focus and assistive technology while retaining their state. Navigator activation and explicit reveals SHALL select their target before positioning it. Hidden files SHALL start no automatic preview work. Without host selection, all sections and scroll-driven navigation SHALL behave as before.

#### Scenario: Visibility does not reset the model
- **WHEN** the host selects another file of the same model and then returns
- **THEN** loaded diffs, image reads, collapse, shown hunks and mounted editor content are retained
- **AND** hidden file sections cannot be tabbed into and start no automatic reads

#### Scenario: Non-selected automatic results arrive
- **WHEN** a read started on the formerly selected file finishes after selection moves
- **THEN** its result is retained for that model without moving the current file or its reading position

#### Scenario: Host selection is absent
- **WHEN** commit detail renders the shared diff without a selected file
- **THEN** every section remains visible and scrolling updates the navigator's current-file marker
