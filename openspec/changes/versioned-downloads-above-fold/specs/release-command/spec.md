# release-command

## MODIFIED Requirements

### Requirement: Commit, Tag, And Push On Approval

On approval the command SHALL write the resolved version into the marketing
site's configuration, commit it together with the notes file, create the target
tag on that commit by invoking the existing version-bump script, and push
`master` together with the tag.

The site constant SHALL be written by the command rather than by the version-bump
script, which creates the tag on an existing commit and therefore runs after the
commit the constant must belong to.

The push SHALL originate from the developer's checkout. A commit pushed by CI
would carry the workflow token, which does not trigger further workflow runs, so
the site would never redeploy.

#### Scenario: Approval commits notes, tags, and pushes

- **WHEN** the user approves at the gate for target version `0.6.0`
- **THEN** the command commits `releases/v0.6.0.md`, creates tag `v0.6.0` on that commit, and pushes `master` and the tag to the remote

#### Scenario: The release commit carries the site version

- **WHEN** the user approves at the gate for target version `0.6.0`
- **THEN** the command SHALL write `0.6.0` into the site's configuration before committing
- **AND** that file SHALL be part of the same commit as the notes file
- **AND** the push SHALL therefore match the site workflow's path filter
