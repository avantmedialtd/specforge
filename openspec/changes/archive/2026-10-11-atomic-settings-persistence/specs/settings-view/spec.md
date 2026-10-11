## ADDED Requirements

### Requirement: The Settings File Is Saved Atomically and in Order

Every write of the settings file SHALL be atomic. The new contents SHALL be written to a uniquely named temporary file in the settings file's directory, flushed to disk, and renamed over the settings file, so that a crash, a failed write or a concurrent writer can never leave a truncated or partly written settings file. A write that fails SHALL leave the previous settings file intact and SHALL leave no temporary file behind. The failure SHALL be returned to the caller and reported as the *Settings Persist by One Rule* requirement describes. It SHALL NOT be swallowed.

Within one running instance, saves SHALL reach the settings file in the order their changes were made. When two changes are made concurrently, for example one from the desktop window and one from a browser tab served by the same instance, the settings file SHALL afterwards hold both changes, and SHALL NOT hold a snapshot taken before the later of them. Reading settings SHALL NOT wait for a save's file I/O to finish.

Separate instances that share one settings file, such as the desktop app and a standalone web server, each keep their own copy of the settings in memory. This requirement does not merge their changes: the last instance to save writes the whole file. Even so, no interleaving of their saves SHALL leave the file unparseable.

#### Scenario: A failed save leaves the previous file intact

- **WHEN** a settings file holding a configured GitHub token exists
- **AND** a later save of the settings fails before the rename completes
- **THEN** the settings file still holds the previous contents, GitHub token included
- **AND** no temporary file is left beside it
- **AND** the control that triggered the save reports the failure

#### Scenario: Concurrent changes both reach disk

- **WHEN** two settings are changed concurrently in the same instance
- **THEN** after both changes return, the settings file on disk holds both new values
- **AND** the settings file equals the settings the instance holds in memory

#### Scenario: Reading settings does not wait on a save

- **WHEN** a settings save is in progress
- **THEN** a read of the current settings returns without waiting for the save's file I/O

### Requirement: A Damaged Settings File Is Never Overwritten by Defaults

Loading the settings file SHALL NOT write to disk except as described here. The outcome SHALL depend on what loading finds:

- **The file is missing.** The settings SHALL load as the defaults, and no file SHALL be created until a setting is changed.
- **The file was read but does not parse as a settings document.** Examples are invalid JSON, an empty file, or a whole-document type mismatch. The file SHALL be moved aside to the first free `settings.json.corrupt-<n>` sibling before anything else happens, so its contents stay recoverable. The condition SHALL be logged, naming the backup path, and the settings SHALL load as the defaults. Later saves, including the first-run identity seed during startup, SHALL write a new settings file and SHALL NOT touch the backup.
- **The file exists but cannot be read.** Examples are a permission error or an I/O error. The file SHALL be left in place, untouched. The condition SHALL be logged, and the settings SHALL load as the defaults for the session. Every save in that session SHALL fail without writing to the settings path, and each failure SHALL be reported on the control the user operated, as the *Settings Persist by One Rule* requirement describes.
- **Moving the damaged file aside fails.** The store SHALL then be treated as unreadable, as in the previous case, so that no save can overwrite the damaged file.

Field-level tolerance SHALL be unchanged. A settings file that a version predating a setting wrote, or one that carries a value this version does not recognise for a field that tolerates unknown values, SHALL load without being moved aside. Examples are an unrecognised panel position and an unrecognised reading width.

#### Scenario: A missing settings file creates nothing

- **WHEN** the application starts and no settings file exists
- **THEN** the settings are the defaults
- **AND** loading creates no settings file

#### Scenario: A settings file that fails to parse is moved aside before startup saves

- **WHEN** the application starts with a settings file that is truncated JSON
- **AND** a registered workspace has a detectable git identity, so the first-run identity seed saves the settings
- **THEN** the truncated file is preserved byte for byte as `settings.json.corrupt-0`
- **AND** a new settings file holding the seeded identity is written
- **AND** the condition is logged with the backup path

#### Scenario: An existing backup is not overwritten

- **WHEN** `settings.json.corrupt-0` already exists
- **AND** the application starts with a settings file that fails to parse
- **THEN** the damaged file is moved to `settings.json.corrupt-1`
- **AND** `settings.json.corrupt-0` is unchanged

#### Scenario: An unreadable settings file is never saved over

- **WHEN** the application starts and the settings file exists but reading it fails with an error other than not-found
- **THEN** the settings for the session are the defaults
- **AND** the file is neither moved nor modified
- **AND** turning on a settings switch reports a failed write on that switch
- **AND** the file at the settings path is still unchanged

#### Scenario: A tolerated unknown value does not move the file aside

- **WHEN** the application starts with a settings file whose pull-request panel position is a value this version does not recognise
- **THEN** the file loads with the panel position at its default and every other setting as stored
- **AND** no `.corrupt-<n>` backup is created
