## ADDED Requirements

### Requirement: An Unchanged Links Snapshot Is Not Re-applied

A frontend SHALL re-read the links snapshot at every point *The Pull-Request Links Snapshot* requirement names: each time it re-reads the workspace views, and each time either pull-request snapshot is announced as updated. A re-read of the workspace views SHALL trigger a links re-read even when the views it returned equal those held, because a remote can change while the views stay equal. When a links read returns a snapshot equal to the one held, the frontend SHALL keep the one it holds. No row, chip, panel or window title derived from the links SHALL render again. When the snapshot differs, only the parts showing the changed links SHALL render again.

A later links read SHALL supersede an earlier one still in flight, so an older reply never replaces a newer snapshot. A failed read SHALL leave the held snapshot in place.

#### Scenario: An equal snapshot changes nothing on screen

- **WHEN** a cache notification causes a views read and the links re-read after it returns a snapshot equal to the held one
- **THEN** the links re-read was made
- **AND** no pull-request panel row, change-header chip or instance-switcher marker renders again

#### Scenario: A remote change with equal views still updates the links

- **WHEN** a repository's remote is changed so that a listed pull request now matches one of its worktrees, and the views read that follows returns views equal to those held
- **THEN** the links re-read still happens
- **AND** the pull-request row leads to that worktree after it
