## ADDED Requirements

### Requirement: Release Body Sourced From Versioned Notes File

The release-publication job SHALL render the GitHub Release body from a versioned notes file committed at the tagged ref, at path `releases/<tag>.md` (the tag including its leading `v`). To do so the job SHALL check out the repository at the tagged ref so the file is present. The job SHALL NOT inline a static release body and SHALL NOT rely on GitHub's auto-generated release notes for the body.

#### Scenario: Body comes from the committed notes file

- **WHEN** tag `v0.6.0` triggers the pipeline and `releases/v0.6.0.md` exists at that commit
- **THEN** the published GitHub Release's body is the rendered contents of `releases/v0.6.0.md`

#### Scenario: Publication job checks out the repository

- **WHEN** the release-publication job runs
- **THEN** it checks out the repository at the tagged ref before resolving the notes file
- **AND** the body path `releases/${tag}.md` resolves to the committed file

#### Scenario: Auto-generated notes are not used for the body

- **WHEN** the pipeline publishes a release
- **THEN** the release body is not GitHub's auto-generated commit/PR list and is not a hard-coded inline body
