## MODIFIED Requirements

### Requirement: Provider-Panel Triage

Existing provider panels SHALL retain their provider scopes, sections, counts, collapse controls, open/worktree gestures, PR numbers and canonical active-PR highlighting. The GitHub and BitBucket panels in the desktop application and browser skin SHALL NOT provide a free-text search field or narrow rows by a text query. They SHALL offer one local filter: All, Failing checks, Conflicts or Drafts, plus Review requested for GitHub only. All SHALL show every row in the provider snapshot.

Header totals SHALL remain snapshot totals; active filtering SHALL show matching/total counts and a clear-filter affordance that restores All. No-match results SHALL be distinct from an empty provider list or authentication failure. Filters SHALL NOT trigger provider reads. Initial row order SHALL follow the snapshot; subsequent snapshots SHALL preserve surviving row order within each section, append newcomers in snapshot order and remove missing rows without remounting a focused surviving row. The removal of provider-panel search SHALL NOT change the Files navigator's path search.

#### Scenario: Provider widgets have no search bar
- **WHEN** an expanded GitHub or BitBucket panel lists pull requests in the desktop application or browser skin
- **THEN** its controls contain the existing labelled filter selector without a search field or an empty search row
- **AND** All shows every snapshot row regardless of title, repository, number or branch names

#### Scenario: Finding a requested review
- **WHEN** the reader selects GitHub Review requested
- **THEN** only requested-review rows are listed with matching/total counts while header totals remain snapshot totals
- **AND** no extra provider request occurs

#### Scenario: Clearing a status filter
- **WHEN** the reader activates Clear while a non-default panel filter is selected
- **THEN** All is selected and every snapshot row is shown again
- **AND** matching/total counts and Clear disappear without a provider request

#### Scenario: A filter matches no rows
- **WHEN** a provider snapshot has rows but none match the selected filter
- **THEN** the panel shows a no-match message, zero matching rows, the unchanged snapshot totals and Clear
- **AND** it does not show an empty-provider or authentication-failure message

#### Scenario: A refresh changes urgency
- **WHEN** a surviving row changes from passing to failing checks during a panel refresh
- **THEN** its status updates without moving relative to other surviving rows in its section
- **AND** a focused row retains focus

#### Scenario: Canonical active selection
- **WHEN** the current PR address uses different casing from its panel row
- **THEN** that row remains visibly and accessibly marked as the current PR

#### Scenario: File search is unchanged
- **WHEN** the reader searches a path in the PR Files navigator
- **THEN** the existing case-insensitive path search and review filters remain available
- **AND** provider-panel filtering remains independent of file search
