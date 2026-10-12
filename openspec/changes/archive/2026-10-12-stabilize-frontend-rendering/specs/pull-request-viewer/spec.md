## ADDED Requirements

### Requirement: Review Marks Re-render Only the Marked File

Marking or unmarking a file or a hunk viewed, and including or skipping a file, SHALL render again only the parts of the view whose state the action changed (see the *Review Progress* requirement). Those parts are the acted-on file's section, where its mark in flight shows its new state at once and its stored state then settles, and the header's progress count. Every other file's section SHALL keep its rows' elements and SHALL NOT render again. This applies to each step of the action: the mark in flight, the stored write, the progress read after it, and the mark leaving flight.

A `review-progress-changed` notice from another view SHALL likewise render again only the sections of files whose state the new progress changed. A progress read that changes no file's state SHALL render no section again.

The per-file guarantees of *Review Progress* SHALL be unchanged. A mark in flight shows its new state at once. A refused mark shows why on its own file. Hunk states apply only to the detail whose head and base they name. A completed file collapses once, as *Completing a file* says. A change of the detail's head or base SHALL render every file whose hunk states it changes.

#### Scenario: Marking one hunk leaves other files alone

- **WHEN** a pull request's view shows 30 files and the reader marks one hunk of the 4th file viewed
- **THEN** the hunk's checkbox shows it viewed at once and the hunk folds
- **AND** through the write, the progress read after it and the mark leaving flight, the other 29 sections keep every row's element and do not render again

#### Scenario: A mark from another window renders only that file

- **WHEN** two views show one pull request and the reader marks a file viewed in the other view
- **THEN** in this view that file's mark updates and the header count changes
- **AND** no other file's section renders again

#### Scenario: A refused mark stays on its file

- **WHEN** a mark is refused because the pull request's head moved
- **THEN** the refusal shows on the acted-on file only
- **AND** no other file's section renders again
