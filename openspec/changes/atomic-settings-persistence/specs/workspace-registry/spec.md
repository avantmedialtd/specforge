## ADDED Requirements

### Requirement: Presentation Store Saves Atomically and Survives Damage

Saving the presentation store SHALL be atomic in the same way the *Lossless Registry Migration and Data Preservation* requirement makes saving the registry atomic. The new contents SHALL be written to a uniquely named temporary file in the target's directory, flushed, and renamed over the target, so that a crash or a concurrent writer can never leave a truncated or partly written presentation file. Save failures SHALL be surfaced, not swallowed. A failed save SHALL leave the previous presentation file intact.

A presentation file that cannot be used SHALL NOT be erased or overwritten with an empty store:

- **The file was read but does not parse.** The application shell SHALL move it aside to the first free `presentation.json.corrupt-<n>` sibling, log the condition, and start with an empty presentation store. Later edits to a display name, colour or disabled state SHALL write a new file and SHALL NOT touch the backup.
- **The file exists but cannot be read.** The shell SHALL leave it in place, log the condition, and start with an empty presentation store that refuses to save. Each refused save SHALL be surfaced to the caller like any other save failure, and SHALL NOT write to the presentation path.

A missing presentation file SHALL continue to load as an empty store without creating a file.

#### Scenario: A presentation file that fails to parse is preserved

- **WHEN** the application starts with a presentation file containing invalid JSON
- **THEN** the file is preserved byte for byte as `presentation.json.corrupt-0`
- **AND** every workspace shows its default name and colour and is enabled
- **AND** the condition is logged
- **WHEN** the user then sets a workspace's display name
- **THEN** a new presentation file holding that display name is written
- **AND** `presentation.json.corrupt-0` is unchanged

#### Scenario: An unreadable presentation file is never saved over

- **WHEN** the application starts and the presentation file exists but reading it fails with an error other than not-found
- **THEN** the file is neither moved nor modified
- **AND** setting a workspace's display name fails with a surfaced error
- **AND** the file at the presentation path is still unchanged

#### Scenario: A failed presentation save leaves the previous file intact

- **WHEN** a presentation file holding a workspace's display name and colour exists
- **AND** a later save of the presentation store fails before the rename completes
- **THEN** the presentation file still holds the previous display name and colour
- **AND** no temporary file is left beside it
