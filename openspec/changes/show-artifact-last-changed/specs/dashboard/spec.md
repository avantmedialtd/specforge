## MODIFIED Requirements

### Requirement: Today's Ships Feed

The Dashboard SHALL present a "Today's ships" feed: the changes archived today, aggregated across every registered workspace, ordered newest-archived first. A change SHALL be considered shipped today when its archived directory (`openspec/changes/archive/<YYYY-MM-DD>-<id>/`) is dated to the viewer's local calendar day, consistent with the day boundary used by the commit garden and the *Today's Progress* hero. The feed's membership SHALL be determined from the dated archive directory and SHALL NOT require git. Each feed entry SHALL identify its change — by its title when available, otherwise its change id — and its owning workspace or repository. When the change's archival instant is recoverable from git history, each entry SHALL additionally present a relative archive time (for example, "archived 2h ago"); when it is not recoverable, the entry SHALL render **neither the relative time nor the label that introduces it**, rather than an introduction with nothing after it. Entries SHALL be ordered by archival instant, newest first, falling back to a stable order when the instant is unavailable.

The relative archive time SHALL use the **same relative-time vocabulary** as every other surface in the application that presents an elapsed time — the workspace tree's per-instance modification time and the detail pane's change-identity header (see the `spec-browser` capability). One kind of value SHALL NOT be spelled differently on different surfaces, and this equivalence SHALL hold at every tier of the vocabulary, so that changing how one surface words an interval cannot leave the others behind. The vocabulary SHALL advance without user action wherever it is displayed, so a feed left open does not freeze at the moment it was painted.

#### Scenario: Feed lists changes archived today

- **WHEN** the today's ships feed renders
- **AND** one or more changes were archived to a directory dated the viewer's local today
- **THEN** those changes are listed, newest-archived first
- **AND** changes archived on an earlier day are not listed

#### Scenario: Entry shows a relative archive time when git supplies it

- **WHEN** a shipped change's archival instant is recoverable from git history
- **THEN** its feed entry shows a relative archive time

#### Scenario: An entry with no recoverable instant shows no archive-time text at all

- **WHEN** a shipped change's archival instant is not recoverable from git history
- **THEN** its feed entry renders no relative archive time
- **AND** it renders no introducing label left stranded without a time after it

#### Scenario: The feed words an interval as the rest of the application does

- **WHEN** the Dashboard's ships feed and any other surface presenting an elapsed time are compared
- **THEN** an interval of the same length is rendered in the same words on both
- **AND** this holds at every tier of the vocabulary

#### Scenario: Entry identifies change and workspace

- **WHEN** the today's ships feed renders an entry
- **THEN** the entry shows the change's title when available, otherwise its change id
- **AND** the entry shows the change's owning workspace or repository
