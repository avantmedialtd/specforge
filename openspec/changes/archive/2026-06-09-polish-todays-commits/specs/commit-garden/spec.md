## MODIFIED Requirements

### Requirement: Dormant and Degraded States

A top-level entry with no commits on the current local day SHALL be **omitted**
from the commit-garden section rather than rendering a placeholder. A non-git
(flat) workspace, and any entry whose repository cannot be read because the
`git` binary is unavailable, SHALL likewise be omitted. When **every** registered
entry is dormant in this sense (quiet, non-git, or git-unavailable), the entire
commit-garden section SHALL be omitted, consistent with the empty-registry rule,
rather than rendering an empty area, a lonely heading, or an error. The section
SHALL NOT error when git is absent, and the rest of the Dashboard SHALL continue
to function.

#### Scenario: A quiet workspace is omitted

- **WHEN** a registered repository received no commits on the current local day
- **AND** at least one other registered entry has commits today
- **THEN** the quiet repository's plot is omitted from the section rather than
  shown as a "quiet today" placeholder
- **AND** the entries with commits today are still rendered

#### Scenario: Non-git workspace is omitted

- **WHEN** a registered workspace is not inside a git repository
- **THEN** its plot is omitted from the section

#### Scenario: Every entry quiet omits the section

- **WHEN** no registered entry has any commits on the current local day
- **THEN** the commit-garden section is omitted entirely rather than rendering a
  section of placeholders or a heading with no plots

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** every entry is dormant, so the commit-garden section is omitted
- **AND** the rest of the Dashboard continues to function
