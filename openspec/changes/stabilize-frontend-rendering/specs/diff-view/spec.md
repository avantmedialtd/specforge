## ADDED Requirements

### Requirement: Host Slot Updates Re-render Only Their File

A host's slots and folds for one file SHALL keep their identity for as long as that file's own inputs are unchanged (see the *Diff View Hosts* requirement). This covers the header extra, the preamble, and the hunk heading extra, hunk end and folded hunks. When a host's state changes, the view SHALL render again only the sections of files whose slots or folds changed. Every other section SHALL keep its rows' elements and SHALL NOT render again. This SHALL hold for files shown within the eager budget and for files loaded on request alike (see the *Line and Byte Budgets With On-Request Loading* requirement).

A host change that alters no file's slots or folds SHALL render no section again. It SHALL NOT re-run the view's fold bookkeeping either: the pruning of shown hunks and the reader's-place anchor.

This SHALL NOT weaken the guarantees of the *Folded Hunks* requirement. A change to one file's folds SHALL still drop the shown state of every hunk of that file whose fold changed. It SHALL still keep the reader's place by the table that requirement gives. Slots SHALL still render the same in either layout. A host that passes no slots, such as commit detail, SHALL see no change in what renders.

#### Scenario: A change to one file's slots renders that file alone

- **WHEN** a host shows 40 files and changes the header extra of the 7th file only
- **THEN** the 7th file's section renders with its new header extra
- **AND** the other 39 sections keep every row's element and do not render again

#### Scenario: A host change that touches no slot renders no section

- **WHEN** a host re-renders the view for a reason that changes no file's slots or folds
- **THEN** no file section renders again
- **AND** the shown hunks and the reader's place are left exactly as they were

#### Scenario: A refold still drops the shown state of that file

- **WHEN** the reader showed a folded hunk of one file, the host stopped folding it, and the host then folds it again while every other file's slots stay unchanged
- **THEN** that hunk is folded
- **AND** no other file's section renders again

#### Scenario: A file loaded on request takes part

- **WHEN** a withheld file was loaded on request and the host then changes another file's slots
- **THEN** the loaded file's section does not render again
