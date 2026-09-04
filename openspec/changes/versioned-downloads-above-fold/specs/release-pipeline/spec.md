# release-pipeline

## ADDED Requirements

### Requirement: Site Version Matches Tag

The version recorded in the marketing site's configuration SHALL match the tag
that triggered the release, with the leading `v` stripped. The workflow SHALL
assert this before any platform build starts, and SHALL fail the run when it
does not hold.

Placing the assertion ahead of the builds is normative, not incidental: a
forgotten version bump is the failure this requirement exists to catch, and it
SHALL surface in seconds rather than after the platform builds have run.

#### Scenario: A matching constant lets the release proceed

- **WHEN** the tag `v0.6.0` triggers the pipeline and the site's configuration records `0.6.0`
- **THEN** the assertion SHALL pass and the platform builds SHALL run

#### Scenario: A stale constant fails before the builds

- **WHEN** the tag `v0.6.0` triggers the pipeline and the site's configuration still records `0.5.0`
- **THEN** the workflow SHALL fail and name both values
- **AND** no platform build job SHALL have started
