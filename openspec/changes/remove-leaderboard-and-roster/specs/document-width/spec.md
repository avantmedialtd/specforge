## MODIFIED Requirements

### Requirement: An Unrecognised Reading Width Degrades to the Default

A stored reading width that the running version does not recognise — a settings
file written by a newer version, or one edited by hand — SHALL be treated as the
default rung.

It SHALL NOT fail the load of the settings as a whole. Settings are loaded by
parsing the file in one piece and falling back to the complete defaults when that
parse fails, so a value that could not be deserialized would silently discard
every other preference stored beside it — favourited changes, the developer
identity, tree collapse state, the web-server configuration and the
reader-window geometry. All of those SHALL survive an unrecognised
reading-width value intact.

The frontend SHALL apply the same rule to an unrecognised mirrored value.

The tolerance is read-side only. An unrecognised value SHALL NOT be expected to
survive a write: settings are persisted by serializing the whole record, so the
next write of any setting — including one unrelated to the reading width —
replaces it with the default on disk. A reader who selected a rung on a newer
build and then opens an older one therefore loses that selection permanently
rather than temporarily. This is accepted: what the tolerance exists to protect
is the neighbouring settings, and a preference whose degraded state is a legible
default the reader can re-select is not in the same class as the data beside it.

#### Scenario: An unrecognised value does not survive an unrelated write

- **WHEN** a settings file containing an unrecognised reading width is loaded and any other preference is then changed
- **THEN** the stored reading width is the default rung
- **AND** the unrecognised value is no longer present in the file

#### Scenario: An unknown value loads as the default

- **WHEN** the settings file contains a reading width the running version does not recognise
- **THEN** the reading surface renders at the default rung

#### Scenario: Other settings survive an unknown value

- **WHEN** the settings file contains an unrecognised reading width alongside favourited changes, a developer identity and other preferences
- **THEN** the settings load successfully
- **AND** those preferences are unchanged rather than reset to their defaults
