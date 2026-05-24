# alias-flag

## ADDED Requirements

### Requirement: Long-form flag aliases

The system SHALL accept `--source` and `--destination` as long-form aliases
for the existing `-s` and `-d` flags.

#### Scenario: Source alias accepted

- **WHEN** invoked with `--source ABC`
- **THEN** treated as `-s ABC`
