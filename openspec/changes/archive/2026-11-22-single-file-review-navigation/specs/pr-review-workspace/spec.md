## ADDED Requirements

### Requirement: Single-File Review Presentation

PR Files SHALL show one selected file at a time in both presentations and hosts. Tree activation SHALL select any file, including viewed or skipped files. Marking or filtering SHALL NOT change selection. Existing loads, folds and editors SHALL survive selection changes. Reopening SHALL restore the saved file, falling back to the first listed file if absent. Explicit discussion and draft jumps SHALL select and reveal their target without changing filters.

#### Scenario: Selecting another file
- **WHEN** the reviewer selects a file in the tree
- **THEN** only that file's section is visible and exposed to keyboard and assistive-technology navigation
- **AND** the tree marks it current, and selecting the former file restores its retained loads, folds and editor content

#### Scenario: Marking does not advance
- **WHEN** the reviewer marks the selected file viewed or changes a filter that excludes it
- **THEN** that file stays selected with its existing completion behavior
- **AND** its out-of-filter context remains identifiable until navigation

#### Scenario: Resuming and jumping
- **WHEN** the reviewer reopens a PR at its saved file or jumps to a thread or draft on a different file
- **THEN** that file alone is shown, with obsolete offsets discarded after a push
- **AND** a missing saved file falls back to the first file, while an unlisted or missing jump retains its existing fallback

### Requirement: Bidirectional Unreviewed File Controls

Files SHALL offer visible Previous and Next controls, outside the narrow navigator disclosure, plus the selected file's position. Both directions SHALL traverse matching unreviewed included files in model order, wrapping once and excluding the current file, viewed files and skipped files. Unknown progress or no other eligible target SHALL disable the controls with an accessible explanation. Selection SHALL add no provider read or review write.

#### Scenario: Skipping completed files in either direction
- **WHEN** files in order are unviewed A, viewed B, skipped C and partly viewed D
- **THEN** Next from A selects D and Previous from D selects A
- **AND** Next from D wraps to A and Previous from A wraps to D

#### Scenario: Changed files remain reviewable
- **WHEN** a file's current state is changed since viewed
- **THEN** both directional controls may select it when it matches the active search and filter
- **AND** its previous viewed mark does not exclude its changed patch

#### Scenario: No eligible destination
- **WHEN** progress is unknown or no other matching unreviewed included file exists
- **THEN** both directional controls are disabled and explain why
- **AND** the current file remains shown and direct tree selection still works

#### Scenario: Narrow views retain directional controls
- **WHEN** the navigator is hidden behind its narrow-view disclosure
- **THEN** Previous, Next and the selected file's position remain visible without opening that disclosure
- **AND** the controls retain programmatic names, keyboard focus treatment and coarse-pointer targets
