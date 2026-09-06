## MODIFIED Requirements

### Requirement: Per-Instance Divergence Label

For every `ChangeInstance` that is not on the repository's default branch, the application SHALL compute and display at most one divergence label by comparing the instance's change directory contents against the default-branch instance of the same logical change. The labels are:

- `[diverged]` — the change exists in both the default-branch instance and the non-default instance, but the file contents differ at the byte level.
- `[stale]` — the change is archived on the default branch (under `openspec/changes/archive/`) but is still active in the non-default instance.

If the change does not exist on the default branch at all, or if no default branch is known, or if the contents are identical, the instance SHALL display no divergence label.

Two instances SHALL be recognised as instances of the same logical change whether or not they are archived, and whether or not their archive directories carry a date prefix. An archived instance's directory is named `<YYYY-MM-DD>-<id>` in ordinary use and `<id>` in the legacy un-dated form; the logical change it belongs to is identified by `<id>` in both cases. Comparing an active instance against an archived one SHALL therefore key on the change's bare identifier, never on the archive directory's raw name — keying the two forms differently makes the `[stale]` label unreachable for every dated archive directory, which is the form that occurs in practice.

#### Scenario: Diverged content gets the diverged label

- **WHEN** an instance on a non-default branch has different content under `openspec/changes/<name>/` than the default-branch instance of the same logical change
- **THEN** the instance row displays the `[diverged]` label

#### Scenario: Stale-vs-archive gets the stale label

- **WHEN** the default-branch instance of a logical change is in `openspec/changes/archive/<name>/`
- **AND** a non-default instance of the same logical change is in `openspec/changes/<name>/` (still active)
- **THEN** the non-default instance row displays the `[stale]` label

#### Scenario: Stale label fires against a dated archive directory

- **WHEN** the default-branch instance of a logical change `add-thing` is archived at `openspec/changes/archive/2026-09-05-add-thing/`
- **AND** a non-default instance is still active at `openspec/changes/add-thing/`
- **THEN** the non-default instance row displays the `[stale]` label
- **AND** the date prefix on the archive directory does not prevent the two instances from being recognised as the same logical change

#### Scenario: Branch-only change gets no label

- **WHEN** a logical change has no instance on the default branch (it was created only on a feature branch)
- **THEN** every non-default instance displays no divergence label

#### Scenario: Identical content gets no label

- **WHEN** a non-default instance has byte-identical content to the default-branch instance of the same logical change
- **THEN** the non-default instance row displays no divergence label

#### Scenario: No default branch produces no labels

- **WHEN** the repository has no detected default branch
- **THEN** no instance of any logical change in that repository displays a divergence label
